use proc_macro2::Span;
use punctuated::Punctuated;
use syn::*;

pub fn unbounded_generics(generics: &Generics) -> Generics {
    let mut generics = generics.clone();
    for param in &mut generics.params {
        match param {
            GenericParam::Lifetime(lt) => {
                lt.colon_token = None;
                lt.bounds.clear();
            }
            GenericParam::Const(c) => {
                c.eq_token = None;
                c.default = None;
            }
            GenericParam::Type(t) => {
                t.colon_token = None;
                t.default = None;
                t.eq_token = None;
                t.bounds.clear();
            }
        }
    }
    generics.where_clause = None;
    generics
}

pub fn generics_to_path_args(generics: &Generics) -> PathArguments {
    let mut args = Punctuated::new();
    for param in &generics.params {
        match param {
            GenericParam::Lifetime(lt) => {
                args.push(GenericArgument::Lifetime(lt.lifetime.clone()));
            }
            GenericParam::Const(c) => {
                args.push(GenericArgument::Const(Expr::Path(ExprPath {
                    attrs: Default::default(),
                    qself: None,
                    path: c.ident.clone().into(),
                })));
            }
            GenericParam::Type(t) => {
                args.push(GenericArgument::Type(Type::Path(TypePath {
                    qself: None,
                    path: t.ident.clone().into(),
                })));
            }
        }
    }

    PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: Token![<](Span::call_site()),
        args,
        gt_token: Token![>](Span::call_site()),
    })
}
