//! Audit every cash-post site, including new source files, without original data.
//! A new post must be paired with an AccountPosted route and a feedback test
//! before this inventory is updated. Counting calls (rather than only functions
//! or files) also catches an extra, unreported post in an existing consumer.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use syn::visit::{self, Visit};

#[derive(Default)]
struct Posts {
    function: String,
    module_directory: PathBuf,
    path_directory: PathBuf,
    modules: Vec<(PathBuf, bool)>,
    calls: BTreeMap<String, usize>,
}
fn requires_test(meta: &syn::Meta) -> bool {
    if meta.path().is_ident("test") {
        return true;
    }
    let syn::Meta::List(list) = meta else {
        return false;
    };
    if !list.path.is_ident("all") && !list.path.is_ident("any") {
        return false;
    }
    let parts = list
        .parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
        .unwrap();
    if list.path.is_ident("all") {
        parts.iter().any(requires_test)
    } else {
        parts.iter().all(requires_test)
    }
}
fn test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg") && requires_test(&attribute.parse_args().unwrap()))
    })
}
impl<'ast> Visit<'ast> for Posts {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let attributes = match item {
            syn::Item::Fn(item) => &item.attrs,
            syn::Item::Mod(item) => &item.attrs,
            syn::Item::Impl(item) => &item.attrs,
            _ => return visit::visit_item(self, item),
        };
        if !test_only(attributes) {
            visit::visit_item(self, item);
        }
    }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let previous = self.module_directory.clone();
        let previous_path = self.path_directory.clone();
        if item.content.is_some() {
            self.module_directory.push(item.ident.to_string());
            self.path_directory = self.module_directory.clone();
            visit::visit_item_mod(self, item);
            self.module_directory = previous;
            self.path_directory = previous_path;
            return;
        }
        let explicit = item.attrs.iter().find_map(|attribute| {
            if attribute.path().is_ident("path")
                && let syn::Meta::NameValue(value) = &attribute.meta
                && let syn::Expr::Lit(value) = &value.value
                && let syn::Lit::Str(value) = &value.lit
            {
                Some(value.value())
            } else {
                None
            }
        });
        let explicit_path = explicit.is_some();
        let path = if let Some(path) = explicit {
            self.path_directory.join(path)
        } else {
            let path = self.module_directory.join(format!("{}.rs", item.ident));
            if path.exists() {
                path
            } else {
                self.module_directory
                    .join(item.ident.to_string())
                    .join("mod.rs")
            }
        };
        self.modules.push((path, explicit_path));
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.function, item.sig.ident.to_string());
        visit::visit_item_fn(self, item);
        self.function = previous;
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if test_only(&item.attrs) {
            return;
        }
        let previous = std::mem::replace(&mut self.function, item.sig.ident.to_string());
        visit::visit_impl_item_fn(self, item);
        self.function = previous;
    }
    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        if expression.method == "post" || expression.method == "post_player" {
            *self.calls.entry(self.function.clone()).or_default() += 1;
        }
        visit::visit_expr_method_call(self, expression);
    }
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        // Include UFCS (PlayerAccount::post), not just receiver.post syntax.
        if let syn::Expr::Path(path) = &*expression.func
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "post" || segment.ident == "post_player")
        {
            *self.calls.entry(self.function.clone()).or_default() += 1;
        }
        visit::visit_expr_call(self, expression);
    }
}
fn inventory(
    path: &Path,
    explicit: bool,
    root: &Path,
    seen: &mut BTreeSet<PathBuf>,
    sites: &mut BTreeMap<String, usize>,
) {
    let path = path
        .canonicalize()
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    if !seen.insert(path.clone()) {
        return;
    }
    let source = std::fs::read_to_string(&path).unwrap();
    let mut module_directory = path.parent().unwrap().to_path_buf();
    if !explicit
        && !matches!(
            path.file_name().unwrap().to_str().unwrap(),
            "lib.rs" | "game.rs" | "capture.rs" | "mod.rs"
        )
    {
        module_directory.push(path.file_stem().unwrap());
    }
    // Explicit #[path] modules resolve their own path relative to the source
    // file's directory. Nested conventional modules use the module directory.
    let mut posts = Posts {
        module_directory,
        path_directory: path.parent().unwrap().to_path_buf(),
        ..Posts::default()
    };
    posts.visit_file(&syn::parse_file(&source).unwrap());
    let relative = path
        .strip_prefix(root)
        .unwrap()
        .components()
        .map(|component| component.as_os_str().to_str().unwrap())
        .collect::<Vec<_>>()
        .join("/");
    sites.extend(
        posts
            .calls
            .into_iter()
            .map(|(function, count)| (format!("{relative}::{function}"), count)),
    );
    for (module, explicit) in posts.modules {
        inventory(&module, explicit, root, seen, sites);
    }
}
#[test]
fn every_production_post_site_has_an_audited_feedback_route() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    let mut actual = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for entry in ["src/lib.rs", "src/bin/game.rs", "src/bin/capture.rs"] {
        inventory(&root.join(entry), true, &root, &mut seen, &mut actual);
    }
    let expected: BTreeMap<_, _> = [
        // Commands: immediate AccountPosted, before the consumer resumes.
        ("src/business_conversion.rs::debit", 1),
        ("src/business_sale.rs::apply", 2),
        ("src/business_takeover.rs::apply", 2),
        ("src/construction.rs::apply", 1),
        ("src/live_commands.rs::bind_open", 2),
        ("src/live_commands.rs::demolish", 1),
        ("src/event_cards/effects.rs::post", 1),
        // Free post() forwards to that same card helper; audit new uses too.
        ("src/event_cards/effects.rs::apply", 13),
        ("src/event_cards/effects.rs::utility_rebate", 2),
        // The frontend's post closure dispatches feedback, not account stores.
        ("src/bin/frontend/live_host.rs::account_ui_post", 1),
        ("src/event_cards/targeted.rs::apply", 2),
        // Raw property records: post_player emits AccountPosted synchronously.
        ("src/category_ownership.rs::release", 1),
        ("src/category_ownership.rs::apply", 2),
        // Daily/retail helpers: update_daily_businesses, pay_owners and
        // resident::purchase pass separate postings to level_runtime/session_tick.
        // Covered by synthetic economy and frontend account feedback tests.
        ("src/economy.rs::collect_housing_income", 1),
        ("src/economy.rs::pay_owners", 2),
        ("src/economy.rs::settle_rent", 2),
        ("src/economy.rs::charge_utilities", 1),
        ("src/economy.rs::replenish", 1),
        ("src/economy.rs::purchase_business_record", 1),
        // Original lease advance posts zero only; no sound/arrow is requested.
        ("src/category.rs::release", 1),
    ]
    .into_iter()
    .map(|(site, count)| (site.to_owned(), count))
    .collect();
    assert_eq!(
        actual, expected,
        "cash post sites changed: audit the AccountPosted route and its ordering, add feedback coverage, then update this inventory"
    );
}

#[test]
fn audit_detects_new_posts_inside_existing_functions() {
    let source = syn::parse_file(
        r#"
        fn consumer() { account.post(1., 0)?; }
        #[cfg(test)] mod tests { fn helper() { account.post(2., 0); } }
    "#,
    )
    .unwrap();
    let mut before = Posts::default();
    before.visit_file(&source);
    assert_eq!(before.calls.get("consumer"), Some(&1));
    assert!(!before.calls.contains_key("helper"));
    let source = syn::parse_file(
        r#"
        fn consumer() { account.post(1., 0)?; PlayerAccount::post(account, -1., 0)?; }
    "#,
    )
    .unwrap();
    let mut after = Posts::default();
    after.visit_file(&source);
    assert_eq!(after.calls.get("consumer"), Some(&2));
}
