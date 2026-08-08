//! A session must be told which neighbouring graphs it can reach.
//!
//! Regression test for #375. `graph_root` has been able to query any sibling
//! checkout since #363, but nothing ever revealed that a sibling existed. A
//! session spanning two repos therefore read an empty result as "no such
//! symbol", and — with the shell fallback blocked by the hook — had no way
//! left to find it. These tests pin the selection rule that decides which
//! projects are worth naming.

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod selection {
    use tokensave::global_db::sibling_project_keys;

    fn keys(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_string()).collect()
    }

    #[test]
    fn a_repo_beside_the_served_one_is_offered() {
        // The reported layout: two independent checkouts under one parent.
        let all = keys(&["/workspace/my-service", "/workspace/my-shared-lib"]);
        assert_eq!(
            sibling_project_keys("/workspace/my-service", &all),
            vec!["/workspace/my-shared-lib".to_string()]
        );
    }

    #[test]
    fn the_served_project_never_offers_itself() {
        // `select_graph` rejects a graph_root equal to the served root, so
        // suggesting it would hand the model a guaranteed error.
        let all = keys(&["/workspace/my-service"]);
        assert!(sibling_project_keys("/workspace/my-service", &all).is_empty());
    }

    #[test]
    fn unrelated_checkouts_elsewhere_on_disk_are_not_offered() {
        // Every project the user has ever indexed lives in this table. Listing
        // all of them would be noise, and noise in instructions gets ignored.
        let all = keys(&[
            "/workspace/my-service",
            "/elsewhere/unrelated",
            "/home/user/scratch",
        ]);
        assert!(sibling_project_keys("/workspace/my-service", &all).is_empty());
    }

    #[test]
    fn nested_and_parent_projects_are_not_siblings() {
        // A project inside the served root is already covered by the served
        // graph; the parent is a different question (#226), not this one.
        let all = keys(&[
            "/workspace/my-service",
            "/workspace/my-service/vendor/dep",
            "/workspace",
        ]);
        assert!(sibling_project_keys("/workspace/my-service", &all).is_empty());
    }

    #[test]
    fn a_project_directly_under_the_filesystem_root_has_no_siblings() {
        // Otherwise every top-level checkout would be offered to every other,
        // which is the "unrelated checkouts" case wearing a different hat.
        let all = keys(&["/svc", "/lib"]);
        assert!(sibling_project_keys("/svc", &all).is_empty());
    }

    #[test]
    fn stored_spellings_are_compared_after_normalization() {
        // Rows predate #372's canonicalization, so a trailing separator or a
        // lower-case drive letter must not read as a different directory.
        let all = keys(&["/workspace/my-service/", "/workspace/my-shared-lib"]);
        assert_eq!(
            sibling_project_keys("/workspace/my-service", &all),
            vec!["/workspace/my-shared-lib".to_string()]
        );

        let windows = keys(&[r"d:\workspace\svc", r"D:\workspace\lib"]);
        assert_eq!(
            sibling_project_keys(r"D:\workspace\svc", &windows),
            vec![r"D:\workspace\lib".to_string()]
        );
    }

    #[test]
    fn a_crowded_parent_directory_is_capped() {
        // A shared scratch directory can hold dozens of indexed projects. This
        // list is embedded in tool responses, and an uncapped one overran the
        // response budget outright, truncating the JSON mid-string.
        let paths: Vec<String> = (0..40).map(|i| format!("/scratch/p{i:02}")).collect();
        let offered = sibling_project_keys("/scratch/p00", &paths);
        assert_eq!(offered.len(), tokensave::global_db::MAX_SIBLING_PROJECTS);
        assert_eq!(offered.first().map(String::as_str), Some("/scratch/p01"));
    }

    #[test]
    fn the_offered_order_is_stable() {
        // These names go into the initialize instructions, which are cached by
        // the client; an unstable order would churn them for no reason.
        let all = keys(&["/w/svc", "/w/zeta", "/w/alpha", "/w/mid"]);
        assert_eq!(
            sibling_project_keys("/w/svc", &all),
            vec![
                "/w/alpha".to_string(),
                "/w/mid".to_string(),
                "/w/zeta".to_string()
            ]
        );
    }
}
