//! Packages from several sources that are the same application become one
//! [`App`] with several editions. Pure: a list in, a list out. TODO: the
//! real grouper; this stub makes every package its own app.

use crate::model::*;

pub fn group(packages: Vec<Package>, _query: &str) -> Vec<App> {
    packages
        .into_iter()
        .map(|p| App {
            key: format!("{}:{}", p.source.id(), p.id),
            name: p.name.clone(),
            kind: p.kind,
            summary: p.summary.clone(),
            icon: p.icon.clone(),
            developer: p.developer.clone(),
            categories: p.categories.clone(),
            installed: p.installed,
            updated: p.updated,
            popularity: p.popularity,
            relevance: 0.0,
            editions: vec![Edition {
                package: p,
                matched_by: MatchedBy::Alone,
                confidence: 1.0,
            }],
        })
        .collect()
}
