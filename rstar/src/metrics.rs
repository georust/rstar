#[cfg(feature = "debug")]
#[derive(Debug, Default)]
pub struct RTreeMetrics {
    insertions: u64,
    reinsertions: u64,
    choose_subtree: u64,
    choose_subtree_outsiders: u64,
    choose_subtree_leaves: u64,
    recursive_insertions: u64,
    splits: u64,
    resolve_overflow: u64,
    resolve_overflow_overflows: u64,
}

#[cfg(not(feature = "debug"))]
pub struct RTreeMetrics {}

#[cfg(not(feature = "debug"))]
impl RTreeMetrics {
    pub fn increment_insertions(&mut self) {}

    pub fn increment_reinsertions(&mut self) {}

    pub fn increment_choose_subtree(&mut self) {}
    pub fn increment_choose_subtree_outsiders(&mut self) {}
    pub fn increment_choose_subtree_leaves(&mut self) {}

    pub fn increment_splits(&mut self) {}

    pub fn increment_recursive_insertions(&mut self) {}

    pub fn increment_resolve_overflow(&mut self) {}
    pub fn increment_resolve_overflow_overflows(&mut self) {}
}

#[cfg(feature = "debug")]
impl RTreeMetrics {
    pub fn print_stats(&self) {
        let stats = [
            ("Insertions", self.insertions),
            ("reinsertions", self.reinsertions),
            ("choose_subtree", self.choose_subtree),
            ("choose_subtree_outsiders", self.choose_subtree_outsiders),
            ("choose_subtree_leaves", self.choose_subtree_leaves),
            ("recursive_insertions", self.recursive_insertions),
            ("splits", self.splits),
            ("resolve_overflow", self.resolve_overflow),
            (
                "resolve_overflow_overflows",
                self.resolve_overflow_overflows,
            ),
        ];

        println!("RTree Metrics");
        for &(ref description, value) in &stats {
            println!(
                "{}: {} ({:.1}%)",
                description,
                value,
                value as f32 * 100. / self.insertions as f32
            );
        }
    }

    pub fn increment_insertions(&mut self) {
        self.insertions += 1;
    }

    pub fn increment_reinsertions(&mut self) {
        self.reinsertions += 1;
    }

    pub fn increment_choose_subtree(&mut self) {
        self.choose_subtree += 1;
    }

    pub fn increment_choose_subtree_outsiders(&mut self) {
        self.choose_subtree_outsiders += 1;
    }

    pub fn increment_choose_subtree_leaves(&mut self) {
        self.choose_subtree_leaves += 1;
    }

    pub fn increment_splits(&mut self) {
        self.splits += 1;
    }

    pub fn increment_recursive_insertions(&mut self) {
        self.recursive_insertions += 1;
    }

    pub fn increment_resolve_overflow(&mut self) {
        self.resolve_overflow += 1;
    }

    pub fn increment_resolve_overflow_overflows(&mut self) {
        self.resolve_overflow_overflows += 1;
    }
}
