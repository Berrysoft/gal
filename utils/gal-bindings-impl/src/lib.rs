use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_str, Ident, ItemFn};

#[proc_macro_attribute]
pub fn export(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let func = input.clone();
    let func = parse_macro_input!(func as ItemFn);
    let name = func.sig.ident;
    let input = proc_macro2::TokenStream::from(input);
    let export_func = if func.sig.asyncness.is_none() {
        let name_str = name.to_string();
        let expname = parse_str::<Ident>(&format!("__{}", name_str)).unwrap();
        quote! {
            #[doc(hidden)]
            #[export_name = #name_str]
            unsafe extern "C" fn #expname(len: usize, data: *const u8) -> u64 {
                ::gal_bindings::__export(len, data, #name)
            }
            #input
        }
    } else {
        let name_str = format!("{}_async", name);
        let expname = parse_str::<Ident>(&format!("__{}", name_str)).unwrap();
        quote! {
            #[doc(hidden)]
            #[export_name = #name_str]
            unsafe extern "C" fn #expname(len: usize, data: *const u8) -> u64 {
                ::gal_bindings::__export_async(len, data, #name)
            }
            #input
        }
    };
    TokenStream::from(export_func)
}
