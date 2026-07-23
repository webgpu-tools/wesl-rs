use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Expr, Ident, LitInt, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

struct WithAttrs<T> {
    attrs: Vec<Attribute>,
    content: T,
}

pub(crate) struct QueryInput {
    components: Punctuated<QueryComponent, Token![.]>,
}

enum BranchKind {
    Variants,
    Members,
}

enum QueryComponent {
    Variant(Ident, Ident),
    Member(Ident),
    Index(LitInt),
    Branch(BranchKind, Punctuated<WithAttrs<QueryInput>, Token![,]>),
    Iter,
    Expr(Option<Ident>, Expr),
    Void,
}

impl<T: Parse> Parse for WithAttrs<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let content = input.parse()?;
        Ok(Self { attrs, content })
    }
}
impl Parse for QueryComponent {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        if input.peek(Ident) && input.peek2(Token![::]) {
            let enum_name: Ident = input.parse()?;
            input.parse::<Token![::]>()?;
            let variant: Ident = input.parse()?;
            Ok(QueryComponent::Variant(enum_name, variant))
        } else if input.peek(Ident) {
            let member = input.parse()?;
            Ok(QueryComponent::Member(member))
        } else if input.peek(Token![self]) {
            input.parse::<Token![self]>()?;
            Ok(QueryComponent::Member(format_ident!("self")))
        } else if input.peek(LitInt) {
            let index = input.parse()?;
            Ok(QueryComponent::Index(index))
        } else if input.peek(token::Brace) {
            let content;
            braced!(content in input);
            let branch_kind = if content.peek(Ident) && content.peek2(Token![::]) {
                BranchKind::Variants
            } else {
                BranchKind::Members
            };
            let components =
                Punctuated::<WithAttrs<QueryInput>, Token![,]>::parse_separated_nonempty(&content)?;
            content.parse::<Token![,]>().ok();
            Ok(QueryComponent::Branch(branch_kind, components))
        } else if input.peek(token::Bracket) {
            input.parse::<proc_macro2::Group>()?;
            Ok(QueryComponent::Iter)
        } else if input.peek(token::Paren) {
            let content;
            parenthesized!(content in input);
            let ident = if content.peek2(Token![=>]) {
                let ident = content.parse::<Ident>()?;
                content.parse::<Token![=>]>()?;
                Some(ident)
            } else {
                None
            };
            let expr = content.parse::<Expr>()?;
            Ok(QueryComponent::Expr(ident, expr))
        } else if input.is_empty() {
            Ok(QueryComponent::Void)
        } else {
            Err(input.error("expected a struct member, enum variant, `{`, `[` or `(`"))
        }
    }
}

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let components = Punctuated::<QueryComponent, Token![.]>::parse_separated_nonempty(input)?;
        Ok(QueryInput { components })
    }
}

pub(crate) fn query_impl(input: QueryInput, mutable: bool) -> TokenStream {
    fn quote_component(component: QueryComponent, ref_: TokenStream) -> TokenStream {
        match component {
            QueryComponent::Variant(enum_name, variant) => quote! {
                filter_map(|x| match x {
                    #enum_name :: #variant(x) => Some(x),
                    _ => None,
                })
            },
            QueryComponent::Member(member) => {
                quote! { map(|x| #ref_ x.#member) }
            }
            QueryComponent::Index(index) => {
                quote! { map(|x| #ref_ x.#index) }
            }
            QueryComponent::Branch(kind, branch) => match kind {
                BranchKind::Variants => {
                    let cases = branch
                        .into_iter()
                        .filter_map(|WithAttrs { attrs, content }| {
                            let mut iter = content.components.into_iter();
                            let first = match iter.next() {
                                Some(QueryComponent::Variant(enum_name, variant)) => {
                                    quote! { #(#attrs)* #enum_name :: #variant (x) }
                                }
                                _ => return None,
                            };
                            let rest = iter.map(|comp| quote_component(comp, ref_.clone()));
                            Some(quote! { #first => Box::new(std::iter::once(x) #(.#rest)*) })
                        });
                    quote! {
                        flat_map(|x| -> Box<dyn Iterator<Item = _>> {
                            match x {
                                #(#cases,)*
                                _ => Box::new(std::iter::empty()),
                            }
                        })
                    }
                }
                BranchKind::Members => {
                    let cases = branch
                        .into_iter()
                        .filter_map(|WithAttrs { attrs, content }| {
                            let mut iter = content.components.into_iter();
                            let first = match iter.next() {
                                Some(QueryComponent::Member(member)) => {
                                    quote! { std::iter::once(#ref_ x.#member) }
                                }
                                _ => return None,
                            };
                            let rest = iter.map(|comp| quote_component(comp, ref_.clone()));
                            Some(quote! {
                                #(#attrs)*
                                let iter = std::iter::Iterator::chain(iter, #first #(.#rest)*);
                            })
                        });
                    quote! { flat_map(|x| {
                        let iter = std::iter::empty();
                        #(#cases)*
                        iter
                    }) }
                }
            },
            QueryComponent::Iter => {
                quote! { flat_map(|x| { x.into_iter() }) }
            }
            QueryComponent::Expr(ident, expr) => {
                if let Some(ident) = ident {
                    quote! { flat_map(|#ident| { #expr }) }
                } else {
                    quote! { flat_map(#expr) }
                }
            }
            QueryComponent::Void => quote! {},
        }
    }

    let iter = input.components.into_iter().peekable();

    let ref_ = if mutable {
        quote! { &mut }
    } else {
        quote! { & }
    };

    fn prefix(
        ref_: TokenStream,
        it: impl Iterator<Item = QueryComponent>,
    ) -> (TokenStream, impl Iterator<Item = QueryComponent>) {
        let mut it = it.peekable();
        let members = it
            .peeking_take_while(|comp| matches!(comp, QueryComponent::Member(_)))
            .map(|comp| match comp {
                QueryComponent::Member(mem) => mem,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        // let prefix = match it.peek() {
        //     Some(QueryComponent::Branch(BranchKind::Members, branch)) => {
        //         // it.next();
        //         let it = branch.into_iter().flat_map(|b| {
        //             let it = b.components.into_iter().peekable();
        //             prefixes(it).map(|(pre, suf)| (quote! { #(#members).* . #pre }, suf))
        //         });
        //         Box::new(it)
        //     }
        //     _ => Box::new(std::iter::once((
        //         quote! { #(#members).* },
        //         std::iter::empty(),
        //     ))),
        // };
        match members.len() {
            0 => (quote! { std::iter::empty() }, it),
            1 => (quote! { std::iter::once( #(#members).* ) }, it),
            _ => (quote! { std::iter::once( #ref_ #(#members).* ) }, it),
        }
    }

    let (first, iter) = prefix(ref_.clone(), iter);

    let rest = iter.map(|comp| quote_component(comp, ref_.clone()));

    quote! {
        #first #(.#rest)*
    }
}
