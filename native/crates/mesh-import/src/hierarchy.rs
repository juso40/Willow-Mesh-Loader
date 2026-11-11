//! Scene reachability and transforms restricted to selected nodes and their ancestors.

use crate::{Result, Source, fail, math::*};

pub(crate) const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub(crate) struct Hierarchy<'a> {
    nodes: Vec<gltf::Node<'a>>,
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) order: Vec<usize>,
    reachable: Vec<bool>,
    source: &'a Source,
}

impl<'a> Hierarchy<'a> {
    pub(crate) fn read(source: &'a Source) -> Result<Self> {
        let nodes: Vec<_> = source.document().nodes().collect();
        let mut parents = vec![None; nodes.len()];
        let mut reachable = vec![false; nodes.len()];
        let mut order = Vec::new();
        let mut stack: Vec<_> = source.scene()?.nodes().map(|node| (node, None)).collect();
        while let Some((node, parent)) = stack.pop() {
            let index = node.index();
            if reachable[index] {
                return Err(fail("node hierarchy contains a cycle or multiple parents"));
            }
            reachable[index] = true;
            parents[index] = parent;
            order.push(index);
            stack.extend(node.children().map(|child| (child, Some(index))));
        }
        Ok(Self {
            nodes,
            parents,
            order,
            reachable,
            source,
        })
    }

    pub(crate) fn relevant(&self, selected: impl IntoIterator<Item = usize>) -> Result<Vec<bool>> {
        let mut relevant = vec![false; self.nodes.len()];
        for index in selected {
            if !self.reachable[index] {
                return Err(fail(
                    "selected node or skin joint is not reachable from the selected scene",
                ));
            }
            let mut cursor = Some(index);
            while let Some(index) = cursor {
                if relevant[index] {
                    break;
                }
                relevant[index] = true;
                cursor = self.parents[index];
            }
        }
        Ok(relevant)
    }

    pub(crate) fn worlds(
        &self,
        selected: impl IntoIterator<Item = usize>,
    ) -> Result<Vec<Option<Matrix>>> {
        let relevant = self.relevant(selected)?;
        let mut worlds = vec![None; self.nodes.len()];
        for &index in &self.order {
            if relevant[index] {
                self.source.check_extensions(&format!("/nodes/{index}"))?;
                let parent = self.parents[index]
                    .and_then(|parent| worlds[parent])
                    .unwrap_or(IDENTITY);
                worlds[index] = Some(checked_matrix(matrix_mul(
                    parent,
                    node_transform(self.nodes[index].transform())?,
                ))?);
            }
        }
        Ok(worlds)
    }
}
