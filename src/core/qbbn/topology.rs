use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::factor_graph::QBBNGraph;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphTopology {
    /// True when the undirected bipartite variable-factor graph has no cycle.
    pub acyclic: bool,

    /// Number of connected components, including isolated variables.
    pub connected_components: usize,

    pub variable_nodes: usize,
    pub factor_nodes: usize,

    /// Number of variable-factor incidence edges.
    pub edges: usize,
}

impl GraphTopology {
    pub fn kind(&self) -> &'static str {
        let nodes = self.variable_nodes + self.factor_nodes;

        if nodes == 0 {
            "empty"
        } else if !self.acyclic {
            "loopy"
        } else if self.connected_components == 1 {
            "tree"
        } else {
            "forest"
        }
    }

    /// Sum-product BP must match exact inference on a tree or forest.
    pub fn bp_should_be_exact(&self) -> bool {
        self.acyclic
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, node: usize) -> usize {
        if self.parent[node] != node {
            let root = self.find(self.parent[node]);
            self.parent[node] = root;
        }

        self.parent[node]
    }

    /// Returns false when the new edge closes a cycle.
    fn union(&mut self, left: usize, right: usize) -> bool {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);

        if left_root == right_root {
            return false;
        }

        if self.rank[left_root] < self.rank[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }

        self.parent[right_root] = left_root;

        if self.rank[left_root] == self.rank[right_root] {
            self.rank[left_root] += 1;
        }

        true
    }
}

/// Analyze the undirected bipartite factor graph.
///
/// Variables and factors are separate graph nodes. Every factor input and
/// output creates one variable-factor incidence edge.
pub fn analyze_topology(graph: &QBBNGraph) -> GraphTopology {
    let mut node_names = Vec::new();

    for variable_id in graph.variables.keys() {
        node_names.push(format!("variable:{variable_id}"));
    }

    for factor_id in graph.factors.keys() {
        node_names.push(format!("factor:{factor_id}"));
    }

    node_names.sort();

    let node_indices: HashMap<String, usize> = node_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect();

    let mut union_find = UnionFind::new(node_names.len());
    let mut acyclic = true;
    let mut edge_count = 0;

    for factor in graph.factors.values() {
        let factor_name = format!("factor:{}", factor.id);
        let factor_index = node_indices[&factor_name];

        let variable_ids = factor
            .input_ids
            .iter()
            .chain(std::iter::once(&factor.output_id));

        for variable_id in variable_ids {
            let variable_name = format!("variable:{variable_id}");
            let variable_index = node_indices[&variable_name];

            edge_count += 1;

            if !union_find.union(factor_index, variable_index) {
                acyclic = false;
            }
        }
    }

    let mut roots = HashSet::new();

    for node in 0..node_names.len() {
        roots.insert(union_find.find(node));
    }

    GraphTopology {
        acyclic,
        connected_components: roots.len(),
        variable_nodes: graph.variables.len(),
        factor_nodes: graph.factors.len(),
        edges: edge_count,
    }
}
