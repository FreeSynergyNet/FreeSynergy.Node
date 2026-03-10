// schema_form – build a Vec<Box<dyn FormNode>> from a FormSchema.
//
// This is the Ratatui renderer bridge: it reads the static FormSchema
// produced by #[derive(Form)] and instantiates the correct FormNode
// implementation for each field.
//
// Usage:
//   let nodes = schema_form::build_nodes(MyForm::schema(), &prefill, &display_fns, &dynamics, &dyn_opts);
//   let form  = ResourceForm::new(kind, TABS, nodes, edit_id, on_change);

use std::collections::HashMap;

use fsn_form::{FieldMeta, FormSchema, WidgetType};

use crate::ui::form_node::FormNode;
use crate::ui::nodes::{EnvTableNode, MultiSelectInputNode, SectionNode, SelectInputNode, TextAreaNode, TextInputNode};

/// Build form nodes from a static schema.
///
/// # Arguments
/// * `schema`           — Static `FormSchema` from `YourForm::schema()`
/// * `prefill`          — Field-key → value map for edit forms (empty for new forms)
/// * `display_fns`      — Optional human-label mappers for `Select` fields
///                        (key → fn(option_code) -> display_label)
/// * `dynamics`         — Runtime-computed default values (override schema `default_val`)
///                        e.g. `&[("install_dir", format!("{}/fsn", home))]`
/// * `dynamic_options`  — Runtime-computed option lists (override schema `options`)
///                        e.g. `&[("project", project_slugs)]` for a project dropdown
pub fn build_nodes(
    schema:          &FormSchema,
    prefill:         &HashMap<&str, &str>,
    display_fns:     &[(&'static str, fn(&str) -> &'static str)],
    dynamics:        &[(&str, String)],
    dynamic_options: &[(&str, Vec<String>)],
) -> Vec<Box<dyn FormNode>> {
    schema.fields.iter().map(|field| build_node(field, prefill, display_fns, dynamics, dynamic_options)).collect()
}

fn build_node(
    field:           &FieldMeta,
    prefill:         &HashMap<&str, &str>,
    display_fns:     &[(&'static str, fn(&str) -> &'static str)],
    dynamics:        &[(&str, String)],
    dynamic_options: &[(&str, Vec<String>)],
) -> Box<dyn FormNode> {
    let pre_val: Option<&str> = prefill.get(field.key).copied();
    let dyn_val: Option<&str> = dynamics.iter().find(|(k, _)| *k == field.key).map(|(_, v)| v.as_str());

    match field.widget {
        WidgetType::Section => {
            // Separator node — no value, never focused.
            Box::new(SectionNode::new(field.key, field.label_key, field.tab))
        }

        WidgetType::MultiSelect => {
            let display_fn = display_fns.iter().find(|(k, _)| *k == field.key).map(|(_, f)| *f);
            let options: Vec<String> = dynamic_options
                .iter()
                .find(|(k, _)| *k == field.key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| field.options.iter().map(|&s| s.to_string()).collect());
            let mut node = MultiSelectInputNode::new(
                field.key, field.label_key, field.tab, field.required, options,
            );
            if let Some(f) = display_fn { node = node.display(f); }
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(v) = pre_val.or(dyn_val).or(field.default_val) {
                node = node.default_val(v);
            }
            node = node.col(field.col).min_w(field.min_w);
            Box::new(node)
        }

        WidgetType::Select => {
            let display_fn = display_fns.iter().find(|(k, _)| *k == field.key).map(|(_, f)| *f);
            let options: Vec<String> = dynamic_options
                .iter()
                .find(|(k, _)| *k == field.key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| field.options.iter().map(|&s| s.to_string()).collect());
            let mut node = SelectInputNode::new(
                field.key, field.label_key, field.tab, field.required, options,
            );
            if let Some(f) = display_fn { node = node.display(f); }
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(v) = pre_val.or(dyn_val).or(field.default_val) {
                node = node.default_val(v);
            }
            node = node.col(field.col).min_w(field.min_w);
            Box::new(node)
        }

        WidgetType::Password => {
            let mut node = TextInputNode::new(field.key, field.label_key, field.tab, field.required)
                .secret()
                .col(field.col).min_w(field.min_w);
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(n) = field.max_len  { node = node.max_len(n); }
            node = apply_text_value(node, pre_val, dyn_val, field.default_val);
            Box::new(node)
        }

        WidgetType::TextArea => {
            let mut node = TextAreaNode::new(field.key, field.label_key, field.tab, field.required);
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(r) = field.rows     { node = node.rows(r); }
            if let Some(v) = pre_val { node = node.pre_filled(v); }
            else if let Some(v) = dyn_val.or(field.default_val) { node = node.default_val(v); }
            Box::new(node)
        }

        WidgetType::EnvTable => {
            let mut node = EnvTableNode::new(field.key, field.label_key, field.tab);
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(r) = field.rows     { node = node.rows(r); }
            if let Some(v) = pre_val        { node.set_value(v); }
            Box::new(node)
        }

        // Text, Email, IpAddress, Number, Toggle, DirPicker — TextInputNode.
        // DirPicker renders identically for now; future: F2 opens dir browser popup.
        _ => {
            let mut node = TextInputNode::new(field.key, field.label_key, field.tab, field.required)
                .col(field.col).min_w(field.min_w);
            if let Some(h) = field.hint_key { node = node.hint(h); }
            if let Some(n) = field.max_len  { node = node.max_len(n); }
            node = apply_text_value(node, pre_val, dyn_val, field.default_val);
            Box::new(node)
        }
    }
}

/// Apply the highest-priority value to a TextInputNode.
/// Priority: prefill (edit mode, marks dirty) > dynamic default > schema default.
fn apply_text_value(
    node:       TextInputNode,
    pre_val:    Option<&str>,
    dyn_val:    Option<&str>,
    schema_def: Option<&'static str>,
) -> TextInputNode {
    if let Some(v) = pre_val {
        node.pre_filled(v)
    } else if let Some(v) = dyn_val {
        node.default_val(v)
    } else if let Some(v) = schema_def {
        node.default_val(v)
    } else {
        node
    }
}
