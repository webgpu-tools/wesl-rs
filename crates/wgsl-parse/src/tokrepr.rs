//! Turn an instance into a `TokenStream` that represents the instance.

use tokrepr::TokRepr;
use tokrepr::proc_macro2::TokenStream;
use tokrepr::quote::{format_ident, quote};

use crate::{SyntaxNode, span::Spanned, syntax::*};

/// named nodes are those that have an identifier. We use this trait to conditionally
/// implement TokRepr for Spanned<NamedNode>, which allows code injection in `quote_*!`
/// macros from the `wesl` crate.
trait NamedNode {
    fn name(&self) -> Option<String>;
}

impl NamedNode for GlobalDeclaration {
    fn name(&self) -> Option<String> {
        self.ident().map(|id| id.to_string())
    }
}

impl NamedNode for StructMember {
    fn name(&self) -> Option<String> {
        Some(self.ident.to_string())
    }
}

impl NamedNode for Attribute {
    fn name(&self) -> Option<String> {
        if let Attribute::Custom(attr) = self {
            Some(attr.name.to_string())
        } else {
            None
        }
    }
}

impl NamedNode for Expression {
    fn name(&self) -> Option<String> {
        if let Expression::TypeOrIdentifier(ty) = self {
            Some(ty.ident.to_string())
        } else {
            None
        }
    }
}

impl NamedNode for Statement {
    fn name(&self) -> Option<String> {
        // COMBAK: this nesting is hell. Hopefully if-let chains will stabilize soon.
        if let Statement::Compound(stmt) = self
            && stmt.statements.is_empty()
            && let [attr] = stmt.attributes.as_slice()
            && let Attribute::Custom(attr) = attr.node()
        {
            return Some(attr.name.to_string());
        }
        None
    }
}

impl<T: NamedNode + TokRepr> TokRepr for Spanned<T> {
    fn tok_repr(&self) -> TokenStream {
        let node = self.node().tok_repr();
        let span = self.span().tok_repr();

        if let Some(name) = self.name()
            && let Some(suffix) = name.strip_prefix("#")
        {
            let ident = format_ident!("{}", suffix);

            return quote! {
                Spanned::new(#ident.to_owned().into(), #span)
            };
        }

        quote! {
            Spanned::new(#node, #span)
        }
    }
}

impl TokRepr for Ident {
    fn tok_repr(&self) -> TokenStream {
        let name = self.name();
        if let Some(name) = name.strip_prefix("#") {
            let ident = format_ident!("{}", name);
            quote! {
                Ident::from(#ident.to_owned())
            }
        } else {
            let name = name.as_str();
            quote! {
                Ident::new(#name.to_string())
            }
        }
    }
}
