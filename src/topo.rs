use crate::config::Config;
use std::collections::{HashMap, HashSet, VecDeque};

/// Result of a topological sort: successfully sorted packages and any cycle participants.
#[derive(Debug)]
pub struct TopologicalResult {
    /// Packages in valid dependency order.
    pub sorted: Vec<String>,
    /// Packages involved in a circular dependency (empty when no cycle exists).
    pub cycle: Vec<String>,
}

/// Sort packages in topological order based on `depends_on` relationships.
/// Dependencies come before the packages that depend on them.
/// Only considers dependencies among the given packages.
/// Packages involved in circular dependencies are separated into `cycle`
/// instead of causing an error, so the caller can skip them gracefully.
pub fn topological_sort(
    config: &Config,
    packages: &[String],
) -> Result<TopologicalResult, Box<dyn std::error::Error>> {
    let pkg_set: HashSet<&str> = packages.iter().map(|s| s.as_str()).collect();

    // Build in-degree map and adjacency list (only among requested packages)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut reverse_dependencies: HashMap<&str, Vec<&str>> = HashMap::new();

    for name in &pkg_set {
        in_degree.entry(name).or_insert(0);
    }

    for name in &pkg_set {
        if let Some(pkg_config) = config.packages.get(*name) {
            for dep in &pkg_config.depends_on {
                if pkg_set.contains(dep.as_str()) {
                    *in_degree.entry(name).or_insert(0) += 1;
                    reverse_dependencies
                        .entry(dep.as_str())
                        .or_default()
                        .push(name);
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

        if let Some(dependents) = reverse_dependencies.get(name) {
            let mut next: Vec<&str> = Vec::new();
            for &dependent in dependents {
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

    let mut cycle = Vec::new();
    if result.len() != pkg_set.len() {
        let mut in_cycle: Vec<&str> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg > 0)
            .map(|(&name, _)| name)
            .collect();
        in_cycle.sort();
        cycle = in_cycle.into_iter().map(|s| s.to_string()).collect();
    }

    Ok(TopologicalResult {
        sorted: result,
        cycle,
    })
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
        Config {
            packages: map,
            ..Default::default()
        }
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
        assert_eq!(sut.sorted, vec!["git", "neovim", "zed"]);
        assert!(sut.cycle.is_empty());
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
        assert_eq!(sut.sorted, vec!["git", "neovim"]);
        assert!(sut.cycle.is_empty());
    }

    #[test]
    fn test_chain_dependency_orders_correctly() {
        // Arrange — c depends on b, b depends on a
        let config = fixture_config(vec![("c", vec!["b"]), ("b", vec!["a"]), ("a", vec![])]);
        let packages: Vec<String> = vec!["c", "b", "a"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut.sorted, vec!["a", "b", "c"]);
        assert!(sut.cycle.is_empty());
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
        assert_eq!(sut.sorted[0], "a");
        assert_eq!(sut.sorted[3], "d");
        assert_eq!(sut.sorted[1], "b");
        assert_eq!(sut.sorted[2], "c");
        assert!(sut.cycle.is_empty());
    }

    #[test]
    fn test_circular_dependency_returns_cycle_participants() {
        // Arrange — a depends on b, b depends on a
        let config = fixture_config(vec![("a", vec!["b"]), ("b", vec!["a"])]);
        let packages: Vec<String> = vec!["a", "b"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert!(sut.sorted.is_empty());
        assert_eq!(sut.cycle, vec!["a", "b"]);
    }

    #[test]
    fn test_three_way_circular_dependency_returns_cycle_participants() {
        // Arrange — a -> b -> c -> a
        let config = fixture_config(vec![("a", vec!["b"]), ("b", vec!["c"]), ("c", vec!["a"])]);
        let packages: Vec<String> = vec!["a", "b", "c"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert!(sut.sorted.is_empty());
        assert_eq!(sut.cycle, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_partial_cycle_separates_sorted_and_cycle() {
        // Arrange — a and b form a cycle, c has no deps
        let config = fixture_config(vec![("a", vec!["b"]), ("b", vec!["a"]), ("c", vec![])]);
        let packages: Vec<String> = vec!["a", "b", "c"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — c is sorted normally, a and b are in cycle
        assert_eq!(sut.sorted, vec!["c"]);
        assert_eq!(sut.cycle, vec!["a", "b"]);
    }

    #[test]
    fn test_dependency_outside_requested_set_is_ignored() {
        // Arrange — neovim depends on git, but git is not in the requested set
        let config = fixture_config(vec![("neovim", vec!["git"]), ("git", vec![])]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert — git is not in the set, so neovim has no in-set deps
        assert_eq!(sut.sorted, vec!["neovim"]);
        assert!(sut.cycle.is_empty());
    }

    #[test]
    fn test_empty_package_list() {
        // Arrange
        let config = fixture_config(vec![("neovim", vec![])]);
        let packages: Vec<String> = vec![];

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert!(sut.sorted.is_empty());
        assert!(sut.cycle.is_empty());
    }

    #[test]
    fn test_single_package_no_deps() {
        // Arrange
        let config = fixture_config(vec![("neovim", vec![])]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut.sorted, vec!["neovim"]);
        assert!(sut.cycle.is_empty());
    }

    #[test]
    fn test_package_not_in_config_treated_as_no_deps() {
        // Arrange — package exists in the list but not in config
        let config = fixture_config(vec![]);
        let packages: Vec<String> = vec!["unknown"].into_iter().map(String::from).collect();

        // Act
        let sut = topological_sort(&config, &packages).unwrap();

        // Assert
        assert_eq!(sut.sorted, vec!["unknown"]);
        assert!(sut.cycle.is_empty());
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
        assert_eq!(sut.sorted, vec!["curl", "git", "neovim"]);
        assert!(sut.cycle.is_empty());
    }
}
