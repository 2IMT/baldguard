use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Error, Fields, Ident};

enum FieldType {
    Int,
    Str,
    Bool,
}

struct Field {
    name: Ident,
    ty: FieldType,
    optional: bool,
}

struct Derived {
    name: Ident,
    fields: Vec<Field>,
}

fn parse(input: DeriveInput) -> Result<Derived, Error> {
    let mut result = Derived {
        name: input.ident.clone(),
        fields: Vec::new(),
    };

    let fields = if let Data::Struct(s) = input.data {
        if let Fields::Named(fields) = s.fields {
            fields
        } else {
            return Err(Error::new(
                s.fields.span(),
                "IntoVariables only supports structs with named fields",
            ));
        }
    } else {
        return Err(Error::new(
            input.ident.span(),
            "Cannot derive IntoVariables for non-struct type",
        ));
    };

    result.fields.reserve(fields.named.len());
    for field in fields.named {
        let name = field.ident.expect("Unnamed field in fields.named");
        let mut optional = false;
        let ty = match field.ty.to_token_stream().to_string().as_str() {
            "i64" => FieldType::Int,
            "String" => FieldType::Str,
            "bool" => FieldType::Bool,
            "Option < i64 >" => {
                optional = true;
                FieldType::Int
            }
            "Option < String >" => {
                optional = true;
                FieldType::Str
            }
            "Option < bool >" => {
                optional = true;
                FieldType::Bool
            }
            other => {
                return Err(Error::new(
                    field.ty.span(),
                    format!("Unsupported type {other})"),
                ))
            }
        };

        let field = Field { name, ty, optional };

        result.fields.push(field);
    }

    Ok(result)
}

#[proc_macro_derive(ToVariables)]
pub fn to_variables(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match parse(input) {
        Ok(input) => input,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };

    let name = input.name;
    let mut assignments = Vec::new();

    for field in input.fields {
        let field_name = field.name;
        let put = match field.ty {
            FieldType::Int => {
                quote! {
                    result.put(std::stringify!(#field_name).to_string(), baldguard_language::evaluation::Value::Int(value));
                }
            }
            FieldType::Str => {
                quote! {
                    result.put(std::stringify!(#field_name).to_string(), baldguard_language::evaluation::Value::Str(value));
                }
            }
            FieldType::Bool => {
                quote! {
                    result.put(std::stringify!(#field_name).to_string(), baldguard_language::evaluation::Value::Bool(value));
                }
            }
        };

        let assignment = if field.optional {
            quote! {
                if let Some(value) = self.#field_name {
                    #put
                } else {
                    result.put(std::stringify!(#field_name).to_string(), baldguard_language::evaluation::Value::Empty);
                }
            }
        } else {
            quote! {
                let value = self.#field_name;
                #put
            }
        };

        assignments.push(assignment);
    }

    let output = quote! {
        impl baldguard_language::evaluation::ToVariables for #name {
            fn to_variables(self) -> baldguard_language::evaluation::Variables {
                let mut result = baldguard_language::evaluation::Variables::new();
                #(#assignments)*
                result
            }
        }
    };

    TokenStream::from(output)
}
