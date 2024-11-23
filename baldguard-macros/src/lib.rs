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

fn parse(input: DeriveInput, allow_optional: bool) -> Result<Derived, Error> {
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
                "Only structs with named fields are supported",
            ));
        }
    } else {
        return Err(Error::new(input.ident.span(), "Only structs are supported"));
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

        if !allow_optional && optional {
            return Err(Error::new(
                field.ty.span(),
                "Option fields are not supported",
            ));
        }

        let field = Field { name, ty, optional };

        result.fields.push(field);
    }

    Ok(result)
}

#[proc_macro_derive(ToVariables)]
pub fn to_variables(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match parse(input, true) {
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
                    result.put(::std::stringify!(#field_name).to_string(),
                        ::baldguard_language::evaluation::Value::Int(value));
                }
            }
            FieldType::Str => {
                quote! {
                    result.put(::std::stringify!(#field_name).to_string(),
                        ::baldguard_language::evaluation::Value::Str(value));
                }
            }
            FieldType::Bool => {
                quote! {
                    result.put(::std::stringify!(#field_name).to_string(),
                        ::baldguard_language::evaluation::Value::Bool(value));
                }
            }
        };

        let assignment = if field.optional {
            quote! {
                if let Some(value) = self.#field_name {
                    #put
                } else {
                    result.put(::std::stringify!(#field_name).to_string(),
                        ::baldguard_language::evaluation::Value::Empty);
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

    quote! {
        impl ::baldguard_language::evaluation::ToVariables for #name {
            fn to_variables(self) -> ::baldguard_language::evaluation::Variables {
                let mut result = ::baldguard_language::evaluation::Variables::new();
                #(#assignments)*
                result
            }
        }
    }
    .into()
}

#[proc_macro_derive(SetFromAssignment)]
pub fn set_from_assignment(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = match parse(input, true) {
        Ok(input) => input,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };

    let name = input.name;
    let mut cases = Vec::new();
    for field in input.fields {
        let field_name = field.name;

        let (needed_type, correct_case) = match field.ty {
            FieldType::Int => (
                "int",
                quote! {
                    ::baldguard_language::evaluation::Value::Int(value)
                },
            ),
            FieldType::Str => (
                "str",
                quote! {
                    ::baldguard_language::evaluation::Value::Str(value)
                },
            ),
            FieldType::Bool => (
                "bool",
                quote! {
                    ::baldguard_language::evaluation::Value::Bool(value)
                },
            ),
        };

        let wrong_case = quote! {
            _ => {
                let field_name = ::std::stringify!(#field_name);
                let needed_type = #needed_type;
                return Err(::baldguard_language::evaluation::ValueError::new_other(
                    ::std::format!("variable {} shoud be of type {}", field_name, needed_type)
                ).into());
            },
        };

        let assign = if field.optional {
            quote! {
                match value {
                    #correct_case => {
                        self.#field_name = ::std::option::Option::Some(value);
                    },
                    ::baldguard_language::evaluation::Value::Empty => {
                        self.#field_name = ::std::option::Option::None;
                    },
                    #wrong_case
                }
            }
        } else {
            quote! {
                match value {
                    #correct_case => {
                        self.#field_name = value;
                    },
                    ::baldguard_language::evaluation::Value::Empty => {
                        let field_name = ::std::stringify!(#field_name);
                        return Err(::baldguard_language::evaluation::ValueError::new_other(
                            ::std::format!("variable {} cannot be empty", field_name)
                        ).into());
                    },
                    #wrong_case
                }
            }
        };

        let case = quote! {
            stringify!(#field_name) => {
                #assign
            }
        };

        cases.push(case);
    }

    quote! {
        impl ::baldguard_language::evaluation::SetFromAssignment for #name {
            fn set_from_assignment(&mut self, assignment: ::baldguard_language::tree::Assignment)
            -> Result<(), ::baldguard_language::evaluation::EvaluationError> {
                let variables = ::baldguard_language::evaluation::Variables::new();
                let value = match ::baldguard_language::evaluation::evaluate(&assignment.expression, &variables) {
                    Ok(value) => value,
                    Err(e) => {
                        return Err(e);
                    },
                };

                match assignment.identifier.as_str() {
                    #(#cases),*,

                    identifier => {
                        return Err(
                            ::baldguard_language::evaluation::EvaluationError::UndeclaredIndentifier(
                                identifier.to_string()));
                    }
                }

                Ok(())
            }
        }
    }
    .into()
}
