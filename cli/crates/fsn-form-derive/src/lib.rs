// fsn-form-derive – proc-macro #[derive(Form)]
//
// Generates a static `FormSchema` for any struct annotated with `#[derive(Form)]`.
// Each field may carry `#[form(...)]` attributes to describe its UI behaviour.
//
// Supported attributes (all optional — sensible defaults apply):
//
//   label     = "i18n.key"      — i18n key for the label shown above the field
//   hint      = "i18n.key"      — i18n key for the hint line below the field
//   widget    = "text"          — UI control: text | password | email | ip |
//                                 select | multi_select | toggle | number | textarea
//   required                    — field must be non-empty to submit
//   tab       = 0               — zero-based tab index (default: 0)
//   max_len   = 255             — maximum character count for text-like widgets
//   rows      = 4               — visible row count for textarea widgets
//   default   = "value"         — static default value
//   options   = "a,b,c"         — comma-separated choices for select / multi_select

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, LitInt, LitStr};

// ── Attribute bag ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct FieldAttrs {
    label:       Option<String>,
    hint:        Option<String>,
    widget:      Option<String>,
    required:    bool,
    tab:         usize,
    max_len:     Option<usize>,
    rows:        Option<u16>,
    default_val: Option<String>,
    options:     Vec<String>,
}

fn parse_form_attrs(field: &Field) -> FieldAttrs {
    let mut a = FieldAttrs::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("form") { continue; }
        // parse_nested_meta handles: #[form(key, key = "val", key = 42, ...)]
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("label") {
                let v = meta.value()?;
                a.label = Some(v.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("hint") {
                let v = meta.value()?;
                a.hint = Some(v.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("widget") {
                let v = meta.value()?;
                a.widget = Some(v.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("required") {
                a.required = true;
            } else if meta.path.is_ident("tab") {
                let v = meta.value()?;
                a.tab = v.parse::<LitInt>()?.base10_parse().unwrap_or(0);
            } else if meta.path.is_ident("max_len") {
                let v = meta.value()?;
                a.max_len = Some(v.parse::<LitInt>()?.base10_parse().unwrap_or(255));
            } else if meta.path.is_ident("rows") {
                let v = meta.value()?;
                a.rows = Some(v.parse::<LitInt>()?.base10_parse().unwrap_or(4));
            } else if meta.path.is_ident("default") {
                let v = meta.value()?;
                a.default_val = Some(v.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("options") {
                let v = meta.value()?;
                let s = v.parse::<LitStr>()?.value();
                a.options = s.split(',').map(|p| p.trim().to_string())
                             .filter(|p| !p.is_empty()).collect();
            }
            Ok(())
        });
    }
    a
}

// ── Code generation ───────────────────────────────────────────────────────────

fn widget_tokens(widget: &str) -> Ts2 {
    match widget {
        "password"      => quote! { ::fsn_form::WidgetType::Password },
        "email"         => quote! { ::fsn_form::WidgetType::Email },
        "ip"            => quote! { ::fsn_form::WidgetType::IpAddress },
        "select"        => quote! { ::fsn_form::WidgetType::Select },
        "multi_select"  => quote! { ::fsn_form::WidgetType::MultiSelect },
        "toggle"        => quote! { ::fsn_form::WidgetType::Toggle },
        "number"        => quote! { ::fsn_form::WidgetType::Number },
        "textarea"      => quote! { ::fsn_form::WidgetType::TextArea },
        "env_table"     => quote! { ::fsn_form::WidgetType::EnvTable },
        _               => quote! { ::fsn_form::WidgetType::Text },
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Derive macro that generates `impl Form for YourStruct`.
///
/// The generated implementation returns a `&'static FormSchema` initialised
/// once via `OnceLock`. Call `YourStruct::schema()` freely — it is zero-cost
/// after the first call.
#[proc_macro_derive(Form, attributes(form))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let named_fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => panic!("#[derive(Form)] requires a struct with named fields"),
        },
        _ => panic!("#[derive(Form)] requires a struct"),
    };

    let field_metas: Vec<Ts2> = named_fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let key_str     = field_ident.to_string();
        let attrs       = parse_form_attrs(field);

        // label defaults to the field name (acts as identity key when no i18n needed)
        let label_key   = attrs.label.as_deref().unwrap_or(&key_str).to_string();
        let hint_tokens = match &attrs.hint {
            Some(h) => quote! { Some(#h) },
            None    => quote! { None },
        };
        let widget_str     = attrs.widget.as_deref().unwrap_or("text");
        let widget         = widget_tokens(widget_str);
        let required       = attrs.required;
        let tab            = attrs.tab;
        let max_len_tokens = match attrs.max_len {
            Some(n) => quote! { Some(#n) },
            None    => quote! { None },
        };
        let rows_tokens = match attrs.rows {
            Some(n) => quote! { Some(#n) },
            None    => quote! { None },
        };
        let default_tokens = match &attrs.default_val {
            Some(d) => quote! { Some(#d) },
            None    => quote! { None },
        };
        let opts: Vec<&str> = attrs.options.iter().map(|s| s.as_str()).collect();

        quote! {
            ::fsn_form::FieldMeta {
                key:         #key_str,
                label_key:   #label_key,
                hint_key:    #hint_tokens,
                widget:      #widget,
                required:    #required,
                tab:         #tab,
                max_len:     #max_len_tokens,
                rows:        #rows_tokens,
                default_val: #default_tokens,
                options:     vec![#(#opts),*],
            }
        }
    }).collect();

    quote! {
        impl ::fsn_form::Form for #struct_name {
            fn schema() -> &'static ::fsn_form::FormSchema {
                static SCHEMA: ::std::sync::OnceLock<::fsn_form::FormSchema> =
                    ::std::sync::OnceLock::new();
                SCHEMA.get_or_init(|| ::fsn_form::FormSchema {
                    fields: vec![#(#field_metas),*],
                })
            }
        }
    }.into()
}
