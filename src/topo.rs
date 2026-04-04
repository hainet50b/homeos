use crate::config::Config;
use std::collections::{HashMap, HashSet, VecDeque};

/// Sort packages in topological order based on `depends_on` relationships.
/// Dependencies come before the packages that depend on them.
/// Only considers dependencies among the given packages.
/// Returns an error if a circular dependency is detected.
#[allow(dead_code)]
pub fn topological_sort(
    config: &Config,
    packages: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let pkg_set: HashSet<&str> = packages.iter().map(|s| s.as_str()).collect();

    // Build in-degree map and adjacency list (only among requested packages)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for name in &pkg_set {
        in_degree.entry(name).or_insert(0);
    }

    for name in &pkg_set {
        if let Some(pkg_config) = config.packages.get(*name) {
            for dep in &pkg_config.depends_on {
                if pkg_set.contains(dep.as_str()) {
                    *in_degree.entry(name).or_insert(0) += 1;
                    dependents.entry(dep.as_str()).or_default().push(name);
                }
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();

    // Sort initial queue for deterministic output
    let mut sorted_queue: Vec<&str> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result: Vec<String> = Vec::new();

    while let Some(name) = queue.pop_front() {
        result.push(name.to_string());

        if let Some(deps) = dependents.get(name) {
            let mut next: Vec<&str> = Vec::new();
            for &dependent in deps {
                let deg = in_degree.get_mut(dependent).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next.push(dependent);
                }
            }
            // Sort for deterministic output
            next.sort();
            queue.extend(next);
        }
    }

    if result.len() != pkg_set.len() {
        // Find the cycle participants for a useful error message
        let in_cycle: Vec<&str> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg > 0)
            .map(|(&name, _)| name)
            .collect();
        let mut sorted_cycle: Vec<&str> = in_cycle;
        sorted_cycle.sort();
        return Err(format!(
            "Circular dependency detected among packages: {}",
            sorted_cycle.join(", ")
        )
        .into());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PackageConfig};
    use std::collections::BTreeMap;

    fn fixture_config(packages: Vec<(&str, Vec<&str>)>) -> Config {
        let mut map = BTreeMap::new();
        for (name, deps) in packages {
            map.insert(
                name.to_string(),
                PackageConfig {
                    depends_on: deps.into_iter().map(String::from).collect(),
                    ..Default::default()
                },
            );
        }
        Config { packages: map }
    }

    #[test]
    fn test_no_dependencies_preserves_sorted_order() {
        // Arrange
        let config = fixture_config(vec![("zed", vec![]), ("neovim", vec![]), ("git", vec![])]);
        let packages: Vec<String> = vec!["zed", "neovim", "git"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — no deps, so alphabetical order
        assert_eq!(sut, vec!["git", "neovim", "zed"]);
    }

    #[test]
    fn test_single_dependency_orders_dep_first() {
        // Arrange
        let config = fixture_config(vec![("neovim", vec!["git"]), ("git", vec![])]);
        let packages: Vec<String> = vec!["neovim", "git"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut, vec!["git", "neovim"]);
    }

    #[test]
    fn test_chain_dependency_orders_correctly() {
        // Arrange — c depends on b, b depends on a
        let config = fixture_config(vec![
            ("c", vec!["b"]),
            ("b", vec!["a"]),
            ("a", vec![]),
        ]);
        let packages: Vec<String> = vec!["c", "b", "a"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_diamond_dependency() {
        // Arrange — d depends on b and c, both depend on a
        let config = fixture_config(vec![
            ("d", vec!["b", "c"]),
            ("b", vec!["a"]),
            ("c", vec!["a"]),
            ("a", vec![]),
        ]);
        let packages: Vec<String> = vec!["d", "c", "b", "a"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — a must come first, d must come last
        assert_eq!(sut[0], "a");
        assert_eq!(sut[3], "d");
        // b and c can be in either order, but deterministic sort gives alphabetical
        assert_eq!(sut[1], "b");
        assert_eq!(sut[2], "c");
    }

    #[test]
    fn test_circular_dependency_errors() {
        // Arrange — a depends on b, b depends on a
        let config = fixture_config(vec![("a", vec!["b"]), ("b", vec!["a"])]);
        let packages: Vec<String> = vec!["a", "b"].into_iter().map(String::from).collect();

        // Act
        let result = topological_sort(&config, &packages);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular dependency"));
        assert!(err.contains("a"));
        assert!(err.contains("b"));
    }

    #[test]
    fn test_three_way_circular_dependency_errors() {
        // Arrange — a -> b -> c -> a
        let config = fixture_config(vec![
            ("a", vec!["b"]),
            ("b", vec!["c"]),
            ("c", vec!["a"]),
        ]);
        let packages: Vec<String> = vec!["a", "b", "c"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let result = topological_sort(&config, &packages);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_dependency_outside_requested_set_is_ignored() {
        // Arrange — neovim depends on git, but git is not in the requested set
        let config = fixture_config(vec![("neovim", vec!["git"]), ("git", vec![])]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — git is not in the set, so neovim has no in-set deps
        assert_eq!(sut, vec!["neovim"]);
    }

    #[test]
    fn test_empty_package_list() {
        // Arrange
        let config = fixture_config(vec![("neovim", vec![])]);
        let packages: Vec<String> = vec![];

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert!(sut.is_empty());
    }

    #[test]
    fn test_single_package_no_deps() {
        // Arrange
        let config = fixture_config(vec![("neovim", vec![])]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut, vec!["neovim"]);
    }

    #[test]
    fn test_package_not_in_config_treated_as_no_deps() {
        // Arrange — package exists in the list but not in config
        let config = fixture_config(vec![]);
        let packages: Vec<String> = vec!["unknown"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut, vec!["unknown"]);
    }

    #[test]
    fn test_multiple_dependencies_ordered_correctly() {
        // Arrange — neovim depends on git and curl
        let config = fixture_config(vec![
            ("neovim", vec!["git", "curl"]),
            ("git", vec![]),
            ("curl", vec![]),
        ]);
        let packages: Vec<String> = vec!["neovim", "git", "curl"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — curl and git before neovim
        assert_eq!(sut, vec!["curl", "git", "neovim"]);
    }
}
