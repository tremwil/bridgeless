use proc_macro::TokenStream;
use proc_macro2 as pm2;
use proc_macro_error::{
    abort, abort_call_site, emit_call_site_error, emit_error, proc_macro_error,
};
use quote::{quote, ToTokens};
use syn::*;

mod helpers;

#[proc_macro_error]
#[proc_macro_derive(Class)]
pub fn derive_class(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    todo!()
}

fn consume_offset(attrs: &mut Vec<Attribute>) -> Option<(Attribute, usize)> {
    let mut offset_attrs = Vec::new();
    attrs.retain(|attr| {
        attr.path()
            .is_ident("offset")
            .then(|| offset_attrs.push(attr.clone()))
            .is_some()
    });

    for attr in offset_attrs.iter().skip(1) {
        emit_error!(attr, "duplicate offset attribute is not allowed");
    }

    offset_attrs.first().and_then(|attr| {
        attr.parse_args::<pm2::Literal>()
            .ok()
            .and_then(|lit| str::parse::<usize>(&lit.to_string()).ok().map(|o| (attr.clone(), o)))
            .or_else(|| {
                emit_error!(attr, "must provide valid usize literal as an argument");
                None
            })
    })
}

struct VmtFn {
    fun: TraitItemFn,
    offset: usize,
    receiver_mutability: Option<Token![mut]>,
}

impl VmtFn {
    fn from_trait_def(trait_def: &ItemTrait) -> impl Iterator<Item = Self> + use<'_> {
        let mut offset_counter = 0;
        trait_def.items.iter().filter_map(move |item| match item {
            TraitItem::Fn(fun) => {
                let mut fun = fun.clone();
                let mut offset = offset_counter;
                if let Some((attr, ofs)) = consume_offset(&mut fun.attrs) {
                    if ofs < offset_counter {
                        abort!(attr, "offset must be strictly increasing");
                    }
                    else {
                        offset = ofs;
                    }
                }
                offset_counter = offset + 1;

                let receiver_mutability = match fun.sig.inputs.first() {
                    Some(FnArg::Receiver(r)) => {
                        if (r.colon_token.is_none() && r.reference.is_some()) {
                            r.mutability.clone()
                        }
                        else {
                            emit_error!(
                                r,
                                "virtual function must have &self or &mut self receiver type"
                            );
                            return None;
                        }
                    }
                    _ => {
                        emit_error!(
                            fun.sig,
                            "virtual function must have &self or &mut self receiver type"
                        );
                        return None;
                    }
                };

                Some(VmtFn {
                    offset,
                    fun,
                    receiver_mutability,
                })
            }
            other => {
                emit_error!(other, "class vtable can only contain functions");
                None
            }
        })
    }
}

struct BaseClass {
    meta_path: Path,
    data_path: Path,
    inherit_trait_path: Path,
    vmt_parts_trait_path: Path,
}
impl BaseClass {
    fn from_meta_path(meta_path: &Path) -> Self {
        fn append_path(path: &Path, ident: &str, args: &PathArguments) -> Path {
            let mut path = path.clone();
            path.segments.push(PathSegment {
                ident: Ident::new(ident, pm2::Span::call_site()),
                arguments: args.clone(),
            });
            path
        }

        let mut meta_path = meta_path.clone();
        let args = meta_path
            .segments
            .last_mut()
            .map(|s| std::mem::take(&mut s.arguments))
            .unwrap_or_default();

        Self {
            data_path: append_path(&meta_path, "Data", &args),
            inherit_trait_path: append_path(&meta_path, "InheritTrait", &args),
            vmt_parts_trait_path: append_path(&meta_path, "HasVmtParts", &args),
            meta_path,
        }
    }
}

struct ClassInfo {
    vis: Visibility,
    name: Ident,
    name_with_args: Path,
    generics: Generics,
    unbounded_generics: Generics,
    generic_args: PathArguments,
    bases: Vec<BaseClass>,
    methods: Vec<VmtFn>,
}

impl ClassInfo {
    fn new(trait_def: ItemTrait) -> Self {
        if trait_def.auto_token.is_some() {
            abort!(trait_def.auto_token, "class vtable cannot be auto")
        }
        if trait_def.unsafety.is_some() {
            abort!(trait_def.unsafety, "class vtable cannot be unsafe")
        }

        let generics = trait_def.generics.clone();
        generics.params.iter().for_each(|p| match p {
            GenericParam::Lifetime(_) => emit_error!(p, "class cannot be generic over lifetime"),
            _ => (),
        });

        let generic_args = helpers::generics_to_path_args(&generics);

        let name = trait_def.ident.clone();

        let mut name_with_args = Path::from(name.clone());
        name_with_args.segments.last_mut().unwrap().arguments = generic_args.clone();

        let bases: Vec<_> = trait_def
            .supertraits
            .iter()
            .filter_map(|bound| match bound {
                TypeParamBound::Trait(tr) => Some(BaseClass::from_meta_path(&tr.path)),
                TypeParamBound::Lifetime(lt) => {
                    emit_error!(
                        lt,
                        "Bases must be defined using class meta modules, e.g. MyClass_Meta"
                    );
                    None
                }
                _ => abort_call_site!(
                    "Bases must be defined using class meta modules, e.g. MyClass_Meta"
                ),
            })
            .collect();

        Self {
            vis: trait_def.vis.clone(),
            name,
            name_with_args,
            unbounded_generics: helpers::unbounded_generics(&generics),
            generics,
            generic_args,
            bases,
            methods: VmtFn::from_trait_def(&trait_def).collect(),
        }
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn class(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        abort_call_site!("class macro does not take any arguments");
    }

    let class = ClassInfo::new(parse_macro_input!(item));

    let mut stream = pm2::TokenStream::new();
    stream.extend(generate_meta(&class));

    stream.into()
}

fn generate_meta(class: &ClassInfo) -> pm2::TokenStream {
    let vis = &class.vis;
    let name = &class.name;
    let generics = &class.generics;
    let unbounded_generics = &class.unbounded_generics;
    let name_with_args = &class.name_with_args;

    let meta_ident = Ident::new(&(class.name.to_string() + "_Meta"), pm2::Span::call_site());

    let inherit_bounds = class.bases.iter().map(|base| &base.inherit_trait_path);
    let base_vmt_parts = class.bases.iter().map(|base| &base.vmt_parts_trait_path);

    let vmt_part_where_bounds = quote! {
        ::bridgeless::internal::VmtPartGen<#name_with_args>
                #(+ ::bridgeless::internal::VmtPartGen<#base_vmt_parts>)*
    };

    let generic_params = &generics.params;
    let generic_predicates = generics.where_clause.as_ref().map(|w| &w.predicates);

    return quote! {
        #[allow(non_snake_case)]
        #vis mod #meta_ident {
            use super::#name;
            pub type Cls #unbounded_generics = #name_with_args;
            pub trait InheritTrait #generics: #(#inherit_bounds)+* {}
            pub trait HasVmtParts #generics: #vmt_part_where_bounds {}
            impl<_bridgeless_T, #generic_params> HasVmtParts #unbounded_generics for _bridgeless_T where
                _bridgeless_T: #vmt_part_where_bounds, #generic_predicates {}
        }
    };
}

// fn check_restrictions(trait_def: &ItemTrait) {
//     // First, make sure we support the trait
//     if trait_def.generics.lt_token.is_some() {
//         panic!("vtable trait cannot be given a lifetime")
//     }
//     if !trait_def.generics.params.empty_or_trailing() {
//         panic!("vtable traits do not support generic parameters yet")
//     }
//     if trait_def.auto_token.is_some() {
//         panic!("vtable trait cannot be auto")
//     }
//     if trait_def.unsafety.is_some() {
//         panic!("vtable trait cannot be unsafe")
//     }
//     if trait_def.supertraits.len() > 1 {
//         panic!("vtable trait can only have a single supertrait")
//     }
//     if trait_def.items.is_empty() {
//         panic!("vtable trait must contain at least one function")
//     }
// }

// fn extract_base_trait(trait_def: &ItemTrait) -> Vec<proc_macro2::TokenStream> {
//     match trait_def.supertraits.first() {
//         None => None,
//         Some(TypeParamBound::Trait(t)) => Some(t.to_token_stream()),
//         Some(_) => panic!(
//             "vtable trait's bounds must be a single trait representing the base class's vtable."
//         ),
//     }
//     .into_iter()
//     .collect()
// }

// fn set_method_abis(trait_def: &mut ItemTrait, abi: &str) {
//     for item in trait_def.items.iter_mut() {
//         if let TraitItem::Fn(fun) = item {
//             // Add "extern C" ABI to the function if not present
//             fun.sig.abi.get_or_insert(Abi {
//                 extern_token: Token![extern](Span::call_site()),
//                 name: Some(LitStr::new(abi, Span::call_site())),
//             });
//         }
//         else {
//             panic!("vtable trait can only contain functions")
//         }
//     }
// }

// fn trait_fn_to_bare_fn(fun: &TraitItemFn) -> TypeBareFn {
//     let lifetimes = fun
//         .sig
//         .generics
//         .lifetimes()
//         .map(|lt| syn::GenericParam::Lifetime(lt.to_owned()));

//     TypeBareFn {
//         lifetimes: syn::parse2(quote! { for <#(#lifetimes),*> }).unwrap(),
//         unsafety: fun.sig.unsafety,
//         abi: fun.sig.abi.clone(),
//         fn_token: Token![fn](Span::call_site()),
//         paren_token: fun.sig.paren_token,
//         inputs: {
//             let mut inputs = Punctuated::new();
//             let mut has_ref_receiver = false;
//             for input in fun.sig.inputs.iter() {
//                 inputs.push(match input {
//                     FnArg::Receiver(r) => {
//                         has_ref_receiver = r.reference.is_some();
//                         BareFnArg {
//                             attrs: r.attrs.clone(),
//                             name: Some((
//                                 Ident::new("this", Span::call_site()),
//                                 Token![:](Span::call_site()),
//                             )),
//                             ty: Type::Reference(TypeReference {
//                                 and_token: Token![&](Span::call_site()),
//                                 lifetime: r.lifetime().cloned(),
//                                 mutability: r.mutability,
//                                 elem: Box::new(parse_quote!(T)),
//                             }),
//                         }
//                     }
//                     FnArg::Typed(arg) => BareFnArg {
//                         attrs: arg.attrs.clone(),
//                         name: match arg.pat.as_ref() {
//                             Pat::Ident(ident) => {
//                                 Some((ident.ident.clone(), Token![:](Span::call_site())))
//                             }
//                             _ => None,
//                         },
//                         ty: *arg.ty.to_owned(),
//                     },
//                 });
//             }
//             if !has_ref_receiver {
//                 panic!(
//                     "vtable trait method \"{0}\" must have &self or &mut self parameter",
//                     fun.sig.ident.to_string()
//                 )
//             }
//             inputs
//         },
//         variadic: None,
//         output: fun.sig.output.clone(),
//     }
// }

// // TODO (WIP): Handle all lifetime edge cases before implementing
// fn sig_to_vtable_thunk(sig: &Signature) -> proc_macro2::TokenStream {
//     let (receiver_mut, receiver_lt) = match sig.inputs.first() {
//         Some(FnArg::Receiver(r)) => (r.mutability.clone(), r.lifetime().cloned()),
//         _ => unreachable!(),
//     };

//     let self_arg: FnArg = syn::parse2(quote! { &self }).unwrap();
//     let t_arg: FnArg = syn::parse2(quote! { this: & #receiver_lt #receiver_mut T }).unwrap();

//     let mut with_t = sig.clone();

//     *with_t.inputs.first_mut().unwrap() = self_arg;
//     with_t.inputs.insert(1, t_arg);
//     with_t.abi = None; // No need for an ABI on the thunk method

//     let ident = &sig.ident;
//     let arg_idents = with_t.inputs.iter().skip(1).map(|arg| match arg {
//         FnArg::Typed(pt) => match pt.pat.as_ref() {
//             Pat::Ident(ident_pat) => ident_pat.ident.clone(),
//             _ => unreachable!(),
//         },
//         _ => unreachable!(),
//     });

//     quote! {
//         #[inline]
//         pub #with_t {
//             (self.#ident)(#(#arg_idents),*)
//         }
//     }
// }

// /// Attribute proc macro that can be used to turn a dyn-compatible trait definition
// /// into a C++ compatible vtable definition.
// ///
// /// For example, say we have a C++ abstract class of the form
// /// ```cpp
// /// struct Obj {
// ///     uint32_t field;
// ///
// ///     virtual ~Obj() = default;
// ///     virtual uint32_t method(uint32_t arg) const noexcept = 0;
// /// };
// /// ```
// ///
// /// This macro then allows us to represent `Obj`'s virtual function table in Rust
// /// and provide our own implementations:
// ///
// /// ```rs
// /// use vtable_rs::{vtable, VPtr};
// ///
// /// #[vtable]
// /// pub trait ObjVmt {
// ///     fn destructor(&mut self) {
// ///         // We can provide a default implementation too!
// ///     }
// ///     fn method(&self, arg: u32) -> u32;
// /// }
// ///
// /// // VPtr implements Default for types that implement the trait, and provides
// /// // a compile-time generated vtable!
// /// #[derive(Default)]
// /// #[repr(C)]
// /// struct RustObj {
// ///     vftable: VPtr<dyn ObjVmt, Self>,
// ///     field: u32
// /// }
// ///
// /// impl ObjVmt for RustObj {
// ///     extern "C" fn method(&self, arg: u32) -> u32 {
// ///         self.field + arg
// ///     }
// /// }
// ///
// /// ```
// ///
// /// `RustObj` could then be passed to a C++ function that takes in a pointer to `Obj`.
// ///
// /// The macro supports single inhertiance through a single trait bound, e.g.
// ///
// /// ```rs
// /// #[vtable]
// /// pub trait DerivedObjVmt: ObjVmt {
// ///     unsafe fn additional_method(&mut self, s: *const c_char);
// /// }
// /// ```
// ///
// /// The vtable layout is fully typed and can be accessed as `<dyn TraitName as
// VmtLayout>::Layout<T>`. /// A `VPtr` can be `Deref`'d into it to obtain the bare function
// pointers and thus call through /// the vtable directly:
// ///
// /// ```rs
// /// let obj = RustObj::default();
// /// let method_impl = obj.vftable.method; // extern "C" fn(&RustObj, u32) -> u32
// /// let call_result = method_impl(obj, 42);
// /// ```
// #[proc_macro_attribute]
// pub fn vtable(_attr: TokenStream, item: TokenStream) -> TokenStream {
//     let mut trait_def: ItemTrait = syn::parse(item).unwrap();

//     check_restrictions(&trait_def);

//     let base_trait = extract_base_trait(&trait_def);

//     // Add 'static lifetime bound to the trait
//     trait_def.supertraits.push(TypeParamBound::Lifetime(Lifetime::new(
//         "'static",
//         Span::call_site(),
//     )));

//     // TODO: generate a #[cfg] to switch to fastcall for x86 windows support
//     set_method_abis(&mut trait_def, "C");

//     let layout_ident = Ident::new(&(trait_def.ident.to_string() + "Layout"), Span::call_site());
//     let signatures: Vec<_> = trait_def
//         .items
//         .iter()
//         .filter_map(|item| {
//             if let TraitItem::Fn(fun) = item {
//                 Some(&fun.sig)
//             }
//             else {
//                 None
//             }
//         })
//         .collect();

//     let trait_ident = &trait_def.ident;
//     let trait_vis = &trait_def.vis;
//     let fn_idents: Vec<_> = signatures.iter().map(|sig| &sig.ident).collect();
//     let bare_fns = trait_def.items.iter().filter_map(|item| match item {
//         TraitItem::Fn(fun) => Some(trait_fn_to_bare_fn(fun)),
//         _ => None,
//     });

//     // Create token stream with base layout declaration if a base trait is present
//     let base_decl = if base_trait.is_empty() {
//         proc_macro2::TokenStream::new()
//     }
//     else {
//         quote! { _base: self._base, }
//     };

//     let base_deref_impl = match base_trait.first() {
//         None => proc_macro2::TokenStream::new(),
//         Some(base) => quote! {
//             impl<T: 'static> ::core::ops::Deref for #layout_ident<T> {
//                 type Target = <dyn #base as ::vtable_rs::VmtLayout>::Layout<T>;

//                 fn deref(&self) -> &Self::Target {
//                     &self._base
//                 }
//             }
//             impl<T: 'static> ::core::ops::DerefMut for #layout_ident<T> {
//                 fn deref_mut(&mut self) -> &mut Self::Target {
//                     &mut self._base
//                 }
//             }
//         },
//     };

//     // TODO: Figure out 100% reliable strategy to adjust lifetimes
//     // so that lifetime inference works as expected in the trait definition
//     //let thunk_impls = signatures.iter().map(|&s| sig_to_vtable_thunk(s));

//     let mut tokens = trait_def.to_token_stream();
//     tokens.extend(quote! {
//         #[repr(C)]
//         #trait_vis struct #layout_ident<T: 'static> {
//             #(_base: <dyn #base_trait as ::vtable_rs::VmtLayout>::Layout<T>,)*
//             #(#fn_idents: #bare_fns,)*
//         }

//         // impl<T: 'static> #layout_ident<T> {
//         //     #(#thunk_impls)*
//         // }

//         impl<T> ::core::clone::Clone for #layout_ident<T> {
//             fn clone(&self) -> Self {
//                 Self {
//                     #base_decl
//                     #(#fn_idents: self.#fn_idents),*
//                 }
//             }
//         }
//         impl<T> ::core::marker::Copy for #layout_ident<T> {}

//         #base_deref_impl

//         unsafe impl ::vtable_rs::VmtLayout for dyn #trait_ident {
//             type Layout<T: 'static> = #layout_ident<T>;
//         }

//         impl<T: #trait_ident> ::vtable_rs::VmtInstance<T> for dyn #trait_ident {
//             const VTABLE: &'static Self::Layout<T> = &#layout_ident {
//                 #(_base: *<dyn #base_trait as ::vtable_rs::VmtInstance<T>>::VTABLE,)*
//                 #(#fn_idents: <T as #trait_ident>::#fn_idents),*
//             };
//         }
//     });

//     tokens.into()
// }
//     tokens.into()
// }
//     tokens.into()
// }
