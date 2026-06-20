use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Meta, Path};

#[proc_macro_derive(XDRSize, attributes(xdr))]
pub fn derive_xdr_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let crate_path = crate_path(&input.attrs);
    let xdr_size = parse_xdr_size_path(&crate_path);

    let original_generics = input.generics.clone();
    let mut impl_generics_src = input.generics.clone();

    for param in impl_generics_src.type_params_mut() {
        param.bounds.push(syn::parse_quote!(#xdr_size));
    }

    let (impl_generics, _, where_clause) = impl_generics_src.split_for_impl();
    let (_, ty_generics, _) = original_generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => struct_body(&crate_path, &data.fields),
        Data::Enum(data) => enum_body(&crate_path, name, data),
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "XDRSize cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    quote! {
        impl #impl_generics #xdr_size for #name #ty_generics #where_clause {
            fn xdr_size(&self) -> usize {
                #body
            }
        }
    }
    .into()
}

fn crate_path(attrs: &[syn::Attribute]) -> Path {
    for attr in attrs {
        if !attr.path().is_ident("xdr") {
            continue;
        }

        let Meta::List(list) = &attr.meta else {
            continue;
        };

        let mut crate_name_override = None;

        for nested in list
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            .unwrap_or_default()
        {
            let Meta::NameValue(name_value) = nested else {
                continue;
            };

            if !name_value.path.is_ident("crate") {
                continue;
            }

            let syn::Expr::Lit(expr_lit) = name_value.value else {
                continue;
            };

            let syn::Lit::Str(lit_str) = expr_lit.lit else {
                continue;
            };

            crate_name_override = Some(lit_str);
        }

        if let Some(lit_str) = crate_name_override {
            return syn::parse_str::<Path>(&lit_str.value())
                .unwrap_or_else(|_| default_crate_path());
        }
    }

    default_crate_path()
}

fn default_crate_path() -> Path {
    match crate_name("nfs_mamont") {
        Ok(FoundCrate::Itself) => syn::parse_str("crate").expect("crate path"),
        Ok(FoundCrate::Name(name)) => syn::parse_str(&name)
            .unwrap_or_else(|_| syn::parse_str("nfs_mamont").expect("nfs_mamont path")),
        Err(_) => syn::parse_str("crate").expect("crate path"),
    }
}

fn path_to_string(path: &Path) -> String {
    quote!(#path).to_string().replace(' ', "")
}

fn parse_xdr_size_path(crate_path: &Path) -> Path {
    syn::parse_str::<Path>(&format!("{}::xdr::XDRSize", path_to_string(crate_path)))
        .expect("xdr size path")
}

fn struct_body(crate_path: &Path, fields: &Fields) -> proc_macro2::TokenStream {
    let xdr_size = parse_xdr_size_path(crate_path);

    match fields {
        Fields::Named(fields) => {
            let sizes = fields.named.iter().map(|field| {
                let ident = field.ident.as_ref().expect("named field");
                quote! { #xdr_size::xdr_size(&self.#ident) }
            });

            quote! {
                0 #(+ #sizes)*
            }
        }

        Fields::Unnamed(fields) => {
            let sizes = fields.unnamed.iter().enumerate().map(|(idx, _)| {
                let index = syn::Index::from(idx);
                quote! { #xdr_size::xdr_size(&self.#index) }
            });

            quote! {
                0 #(+ #sizes)*
            }
        }

        Fields::Unit => quote! { 0 },
    }
}

fn enum_body(
    crate_path: &Path,
    name: &syn::Ident,
    data: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    let xdr_size = parse_xdr_size_path(crate_path);

    let variants = data.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;

        match &variant.fields {
            Fields::Named(fields) => {
                let bindings: Vec<_> = fields
                    .named
                    .iter()
                    .map(|field| field.ident.clone().expect("named enum field"))
                    .collect();

                let sizes = bindings.iter().map(|ident| {
                    quote! { #xdr_size::xdr_size(#ident) }
                });

                quote! {
                    #name::#variant_ident { #( ref #bindings ),* } => {
                        Self::INTEGER #(+ #sizes)*
                    }
                }
            }

            Fields::Unnamed(fields) => {
                let bindings: Vec<_> = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(idx, _)| {
                        syn::Ident::new(&format!("field_{idx}"), proc_macro2::Span::call_site())
                    })
                    .collect();

                let sizes = bindings.iter().map(|ident| {
                    quote! { #xdr_size::xdr_size(#ident) }
                });

                quote! {
                    #name::#variant_ident( #( ref #bindings ),* ) => {
                        Self::INTEGER #(+ #sizes)*
                    }
                }
            }

            Fields::Unit => {
                quote! {
                    #name::#variant_ident => Self::INTEGER
                }
            }
        }
    });

    quote! {
        match self {
            #(#variants),*
        }
    }
}
