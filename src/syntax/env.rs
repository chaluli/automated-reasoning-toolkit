/// A de Bruijn environment -- a persistent, efficient random-access stack.
///
/// Used to traverse quantified formulas, mapping de Bruijn indices to values.
/// Implemented as a skew-binary random access list for O(1) push and O(log n) lookup.
#[derive(Debug, Clone)]
pub struct Env<T> {
    elts: Vec<Elt<T>>,
}

#[derive(Debug, Clone)]
struct Elt<T> {
    size: usize,
    tree: Node<T>,
}

#[derive(Debug, Clone)]
enum Node<T> {
    Leaf(T),
    Branch(T, Box<Node<T>>, Box<Node<T>>),
}

impl<T: Clone> Env<T> {
    /// Create an empty environment.
    pub fn empty() -> Self {
        Self { elts: Vec::new() }
    }

    /// Push a value onto the front (binding de Bruijn index 0).
    pub fn push(&self, val: T) -> Self {
        let mut new_elts = Vec::with_capacity(self.elts.len() + 1);

        // Skew-binary: if the first two trees have equal size, merge them.
        match (self.elts.first(), self.elts.get(1)) {
            (Some(first), Some(second)) if first.size == second.size => {
                let merged = Elt {
                    size: 2 * first.size + 1,
                    tree: Node::Branch(
                        val,
                        Box::new(first.tree.clone()),
                        Box::new(second.tree.clone()),
                    ),
                };
                new_elts.push(merged);
                if let Some(rest) = self.elts.get(2..) {
                    new_elts.extend_from_slice(rest);
                }
            }
            _ => {
                new_elts.push(Elt {
                    size: 1,
                    tree: Node::Leaf(val),
                });
                new_elts.extend_from_slice(&self.elts);
            }
        }

        Self { elts: new_elts }
    }

    /// Look up a de Bruijn index. Returns `None` if out of bounds.
    pub fn get(&self, mut index: usize) -> Option<&T> {
        for elt in &self.elts {
            if index < elt.size {
                return find_tree(&elt.tree, index, elt.size);
            }
            index -= elt.size;
        }
        None
    }

    /// Iterate over all values in order (index 0 first).
    pub fn iter(&self) -> EnvIter<'_, T> {
        let mut pending = Vec::new();
        for elt in &self.elts {
            pending.push(&elt.tree);
        }
        EnvIter { pending }
    }

    /// Check if the environment is empty.
    pub fn is_empty(&self) -> bool {
        self.elts.is_empty()
    }

    /// Return the number of bindings.
    pub fn len(&self) -> usize {
        self.elts.iter().map(|e| e.size).sum()
    }
}

fn find_tree<T>(node: &Node<T>, index: usize, size: usize) -> Option<&T> {
    match node {
        Node::Leaf(val) => {
            if index == 0 {
                Some(val)
            } else {
                None
            }
        }
        Node::Branch(val, left, right) => {
            if index == 0 {
                Some(val)
            } else {
                let half = size / 2;
                if index <= half {
                    find_tree(left, index - 1, half)
                } else {
                    find_tree(right, index - half - 1, half)
                }
            }
        }
    }
}

/// Iterator over environment entries.
#[derive(Debug)]
pub struct EnvIter<'a, T> {
    /// Pending tree nodes to visit (in-order traversal).
    pending: Vec<&'a Node<T>>,
}

impl<'a, T> Iterator for EnvIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.pending.first().copied()?;
        self.pending.remove(0);
        match node {
            Node::Leaf(val) => Some(val),
            Node::Branch(val, left, right) => {
                // Insert children at the front for in-order traversal
                self.pending.insert(0, right);
                self.pending.insert(0, left);
                Some(val)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let env = Env::<i32>::empty();
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
        assert_eq!(env.get(0), None);
    }

    #[test]
    fn test_push_and_get() {
        let env = Env::empty().push(10).push(20).push(30);
        assert_eq!(env.get(0), Some(&30));
        assert_eq!(env.get(1), Some(&20));
        assert_eq!(env.get(2), Some(&10));
        assert_eq!(env.get(3), None);
        assert_eq!(env.len(), 3);
    }

    #[test]
    fn test_many_elements() {
        let mut env = Env::empty();
        for i in 0..100 {
            env = env.push(i);
        }
        for i in 0..100 {
            assert_eq!(env.get(i), Some(&(99 - i)));
        }
        assert_eq!(env.len(), 100);
    }

    #[test]
    fn test_iter() {
        let env = Env::empty().push(10).push(20).push(30);
        let vals: Vec<_> = env.iter().copied().collect();
        assert_eq!(vals, vec![30, 20, 10]);
    }

    #[test]
    fn test_persistence() {
        let env1 = Env::empty().push(1).push(2);
        let env2 = env1.push(3);
        // env1 is unaffected
        assert_eq!(env1.get(0), Some(&2));
        assert_eq!(env1.len(), 2);
        // env2 has the new element
        assert_eq!(env2.get(0), Some(&3));
        assert_eq!(env2.len(), 3);
    }
}
