use crate::builders::CrateBuilder;
use crate::new_category;
use crate::util::{MockAnonymousUser, RequestHelper, TestApp};
use cargo_registry::models::Category;
use insta::assert_yaml_snapshot;
use serde_json::Value;

#[test]
fn show() {
    let (app, anon) = TestApp::init().empty();
    let url = "/api/v1/categories/foo-bar";

    // Return not found if a category doesn't exist
    anon.get(url).assert_not_found();

    // Create a category and a subcategory
    app.db(|conn| {
        assert_ok!(new_category("Foo Bar", "foo-bar", "Foo Bar crates").create_or_update(conn));
        assert_ok!(
            new_category("Foo Bar::Baz", "foo-bar::baz", "Baz crates").create_or_update(conn)
        );
    });

    // The category and its subcategories should be in the json
    let json: Value = anon.get(url).good();
    assert_yaml_snapshot!(json, {
        ".**.created_at" => "[datetime]",
    });
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn update_crate() {
    // Convenience function to get the number of crates in a category
    fn count(anon: &MockAnonymousUser, category: &str) -> usize {
        let json = anon.show_category(category);
        json.category.crates_cnt as usize
    }

    let (app, anon, user) = TestApp::init().with_user();
    let user = user.as_model();

    app.db(|conn| {
        assert_ok!(new_category("cat1", "cat1", "Category 1 crates").create_or_update(conn));
        assert_ok!(
            new_category("Category 2", "category-2", "Category 2 crates").create_or_update(conn)
        );
        let krate = CrateBuilder::new("foo_crate", user.id).expect_build(conn);

        // Updating with no categories has no effect
        Category::update_crate(conn, &krate, &[]).unwrap();
        assert_eq!(count(&anon, "cat1"), 0);
        assert_eq!(count(&anon, "category-2"), 0);

        // Happy path adding one category
        Category::update_crate(conn, &krate, &["cat1"]).unwrap();
        assert_eq!(count(&anon, "cat1"), 1);
        assert_eq!(count(&anon, "category-2"), 0);

        // Replacing one category with another
        Category::update_crate(conn, &krate, &["category-2"]).unwrap();
        assert_eq!(count(&anon, "cat1"), 0);
        assert_eq!(count(&anon, "category-2"), 1);

        // Removing one category
        Category::update_crate(conn, &krate, &[]).unwrap();
        assert_eq!(count(&anon, "cat1"), 0);
        assert_eq!(count(&anon, "category-2"), 0);

        // Adding 2 categories
        Category::update_crate(conn, &krate, &["cat1", "category-2"]).unwrap();
        assert_eq!(count(&anon, "cat1"), 1);
        assert_eq!(count(&anon, "category-2"), 1);

        // Removing all categories
        Category::update_crate(conn, &krate, &[]).unwrap();
        assert_eq!(count(&anon, "cat1"), 0);
        assert_eq!(count(&anon, "category-2"), 0);

        // Attempting to add one valid category and one invalid category
        let invalid_categories =
            Category::update_crate(conn, &krate, &["cat1", "catnope"]).unwrap();
        assert_eq!(invalid_categories, vec!["catnope"]);
        assert_eq!(count(&anon, "cat1"), 1);
        assert_eq!(count(&anon, "category-2"), 0);

        // Does not add the invalid category to the category list
        // (unlike the behavior of keywords)
        let json = anon.show_category_list();
        assert_eq!(json.categories.len(), 2);
        assert_eq!(json.meta.total, 2);

        // Attempting to add a category by display text; must use slug
        Category::update_crate(conn, &krate, &["Category 2"]).unwrap();
        assert_eq!(count(&anon, "cat1"), 0);
        assert_eq!(count(&anon, "category-2"), 0);

        // Add a category and its subcategory
        assert_ok!(new_category("cat1::bar", "cat1::bar", "bar crates").create_or_update(conn));
        Category::update_crate(conn, &krate, &["cat1", "cat1::bar"]).unwrap();

        assert_eq!(count(&anon, "cat1"), 1);
        assert_eq!(count(&anon, "cat1::bar"), 1);
        assert_eq!(count(&anon, "category-2"), 0);
    });
}
