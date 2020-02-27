extern crate proc_macro;
use crate::proc_macro::TokenStream;
use linked_hash_map::LinkedHashMap;
use quote::quote;
use syn::*;

#[proc_macro_attribute]
pub fn cenum(_metadata: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_cenum(&ast)
}

fn impl_cenum(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let variants = match &ast.data {
        Data::Enum(DataEnum { variants, .. }) => variants.into_iter().collect::<Vec<&Variant>>(),
        _ => panic!("not deriving cenum on an enum"),
    };
    if variants
        .iter()
        .any(|variant| variant.fields != Fields::Unit)
    {
        panic!("cannot have cenum trait on enums with fields")
    }
    let mut pairs: LinkedHashMap<String, usize> = LinkedHashMap::new();
    let mut current_discriminant = 0;
    for variant in &variants {
        let discriminant = match &variant.discriminant {
            Some((
                _,
                Expr::Lit(ExprLit {
                    lit: Lit::Int(lit_int),
                    ..
                }),
            )) => {
                let discriminant = lit_int.base10_parse::<usize>().unwrap();
                if discriminant < current_discriminant {
                    panic!("attempted to reuse discriminant");
                }
                current_discriminant = discriminant + 1;
                discriminant
            }
            Some(_) => panic!("expected integer literal as discriminant"),
            None => {
                let discriminant = current_discriminant;
                current_discriminant += 1;
                discriminant
            }
        };
        pairs.insert(variant.ident.to_string(), discriminant);
    }

    let pairs_formatted = format!(
        "[{}]",
        pairs
            .iter()
            .map({ |(key, value)| format!("({}::{}, {})", name.to_string(), key, value) })
            .collect::<Vec<String>>()
            .join(", ")
    );
    let pairs_parsed: ExprArray = parse_str(&pairs_formatted).unwrap();

    let data_name = Ident::new(
        &format!("__{}_data", name.to_string()).to_uppercase(),
        name.span(),
    );
    let cache_name = Ident::new(
        &format!("__{}_cache", name.to_string()).to_uppercase(),
        name.span(),
    );
    let icache_name = Ident::new(
        &format!("__{}_icache", name.to_string()).to_uppercase(),
        name.span(),
    );
    let get_cache_name = Ident::new(&format!("__{}_get_cache", name.to_string()), name.span());
    let get_icache_name = Ident::new(&format!("__{}_get_icache", name.to_string()), name.span());

    let gen = quote! {

        #[derive(PartialEq, Eq, Hash, Clone, Debug)]
        #ast

        static #data_name: &'static [(#name, usize)] = &#pairs_parsed;
        static mut #cache_name: Option<::cenum::linked_hash_map::LinkedHashMap<#name, usize>> = None;
        static mut #icache_name: Option<::cenum::linked_hash_map::LinkedHashMap<usize, #name>> = None;

        #[allow(non_snake_case)]
        fn #get_cache_name() -> &'static ::cenum::linked_hash_map::LinkedHashMap<#name, usize> {
            unsafe {
                if #cache_name.is_none() {
                    #cache_name = Some(::linked_hash_map::LinkedHashMap::new());
                    for (key, value) in #data_name {
                        #cache_name.as_mut().unwrap().insert(key.clone(), *value);
                    }
                }
                return #cache_name.as_ref().unwrap();
            }
        }

        #[allow(non_snake_case)]
        fn #get_icache_name() -> &'static ::cenum::linked_hash_map::LinkedHashMap<usize, #name> {
            unsafe {
                if #icache_name.is_none() {
                    #icache_name = Some(::cenum::linked_hash_map::LinkedHashMap::new());
                    for (key, value) in #data_name {
                        #icache_name.as_mut().unwrap().insert(*value, key.clone());
                    }
                }
                return #icache_name.as_ref().unwrap();
            }
        }

        impl Cenum for #name {
            fn to_primitive(&self) -> usize {
                return *#get_cache_name().get(self).unwrap();
            }

            fn from_primitive(value: usize) -> #name {
                return #get_icache_name().get(&value).unwrap().clone();
            }

            fn is_discriminant(value: usize) -> bool {
                return #get_icache_name().get(&value).is_some();
            }
        }

        impl ::cenum::num::ToPrimitive for #name {
            fn to_i64(&self) -> Option<i64> {
                Some(self.to_primitive() as i64)
            }

            fn to_u64(&self) -> Option<u64> {
                Some(self.to_primitive() as u64)
            }
        }


    };
    gen.into()
}
