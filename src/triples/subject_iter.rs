use super::{Id, TripleId, TriplesBitmap};

/// Iterator over triples fitting an SPO, SP? S?? or ??? triple pattern.
//#[derive(Debug)]
pub struct SubjectIter<'a> {
    // triples data
    triples: &'a TriplesBitmap,
    // x-coordinate identifier
    x: Id,
    // current position
    pos_y: usize,
    pos_z: usize,
    max_y: usize,
    max_z: usize,
    search_z: usize, // for S?O
}

impl<'a> SubjectIter<'a> {
    /// Create an iterator over all triples.
    pub fn new(triples: &'a TriplesBitmap) -> Self {
        SubjectIter {
            triples,
            x: 1, // was 0 in the old code but it should start at 1
            pos_y: 0,
            pos_z: 0,
            max_y: triples.wavelet_y.len(), // exclusive
            max_z: triples.adjlist_z.len(), // exclusive
            search_z: 0,
        }
    }

    /// Use when no results are found.
    pub const fn empty(triples: &'a TriplesBitmap) -> Self {
        SubjectIter { triples, x: 1, pos_y: 0, pos_z: 0, max_y: 0, max_z: 0, search_z: 0 }
    }

    /// Convenience method for the S?? triple pattern.
    /// See <https://github.com/rdfhdt/hdt-cpp/blob/develop/libhdt/src/triples/BitmapTriplesIterators.cpp>.
    pub fn with_s(triples: &'a TriplesBitmap, subject_id: Id) -> Self {
        let min_y = triples.find_y(subject_id - 1);
        let min_z = triples.adjlist_z.find(min_y as Id);
        let max_y = triples.find_y(subject_id);
        let max_z = triples.adjlist_z.find(max_y as Id);
        SubjectIter { triples, x: subject_id, pos_y: min_y, pos_z: min_z, max_y, max_z, search_z: 0 }
    }

    /// Iterate over triples fitting the given SPO, SP? S??, S?O or ??? triple pattern.
    /// Variable positions are signified with a 0 value.
    /// Undefined result if any other triple pattern is used.
    /// # Examples
    /// ```text
    /// // S?? pattern, all triples with subject ID 1
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 0, 0);
    /// // SP? pattern, all triples with subject ID 1 and predicate ID 2
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 2, 0);
    /// // match a specific triple, not useful in practice except as an ASK query
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 2, 3);
    /// ```
    // Translated from <https://github.com/rdfhdt/hdt-cpp/blob/develop/libhdt/src/triples/BitmapTriplesIterators.cpp>.
    pub fn with_pattern(triples: &'a TriplesBitmap, pat: &TripleId) -> Self {
        let (pat_x, pat_y, pat_z) = (pat.subject_id, pat.predicate_id, pat.object_id);
        let (min_y, max_y, min_z, max_z);
        let mut x = 1;
        let mut search_z = 0;
        // only SPO order is supported currently
        if pat_x != 0 {
            // S X X
            if pat_y != 0 {
                // S P X
                match triples.search_y(pat_x - 1, pat_y) {
                    Some(y) => min_y = y,
                    None => return SubjectIter::empty(triples),
                }
                max_y = min_y + 1;
                if pat_z != 0 {
                    // S P O
                    // simply with try block when they come to stable Rust
                    match triples.adjlist_z.search(min_y, pat_z) {
                        Some(pos_z) => min_z = pos_z,
                        None => return SubjectIter::empty(triples),
                    }
                    max_z = min_z + 1;
                } else {
                    // S P ?
                    min_z = triples.adjlist_z.find(min_y);
                    max_z = triples.adjlist_z.last(min_y) + 1;
                }
            } else {
                // S ? X
                min_y = triples.find_y(pat_x - 1);
                min_z = triples.adjlist_z.find(min_y);
                max_y = triples.last_y(pat_x - 1) + 1;
                max_z = triples.adjlist_z.find(max_y);
                search_z = pat_z;
            }
            x = pat_x;
        } else {
            // ? X X
            // assume ? ? ?, other triple patterns are not supported by this iterator
            min_y = 0;
            min_z = 0;
            max_y = triples.wavelet_y.len();
            max_z = triples.adjlist_z.len();
        }
        SubjectIter { triples, x, pos_y: min_y, pos_z: min_z, max_y, max_z, search_z }
    }

    /// While building the pattern, allows jumping to an offset efficiently.
    /// This avoids the need to iterate over every element until reaching the desired
    /// offset.
    pub fn with_pattern_and_offset(triples: &'a TriplesBitmap, pat: &TripleId, op_offset: Option<usize>) -> SubjectIter<'a> {
        let mut base = SubjectIter::with_pattern(triples, pat);
        match op_offset {
            None => base, // regular
            Some(offset) => { // with jump
                if base.search_z > 0 { // TODO make sure that it can't be skipped
                    (0..offset).for_each(|_i| {base.next();});
                    return base;
                }
                // comes from:
                // <https://github.com/rdfhdt/hdt-cpp/blob/d9ae092bb37d9fe85558dfb3edfe0bb6ddddf41a/libhdt/src/triples/BitmapTriplesIterators.cpp#L258>
                let max_z: usize = triples.adjlist_z.len();
                assert!(base.pos_z + offset < max_z); // TODO not throw, but be empty ?
                let pos_z = base.pos_z + offset;

                // BitmapTriplesSearchIterator::goToY
                let mut pos_y = triples.adjlist_z.bitmap.rank(pos_z - 1) as Id;
                // let z = triples.adjlist_z.sequence.get(pos_z - 1) as Id;
                // println!("{}", z);
                // let y = triples.adjlist_y.sequence.get(pos_y);
                // println!("{}", y);
                // TODO if adjlist_y.sequence of no use, we can revert change in triples
                let x = triples.bitmap_y.rank(pos_y - 1) as Id;

                SubjectIter {
                    triples,
                    x,
                    pos_y,
                    pos_z,
                    max_y: base.max_y,
                    max_z: base.max_z,
                    search_z: 0,
                }
            }
        }
    }
}

impl Iterator for SubjectIter<'_> {
    type Item = TripleId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos_y >= self.max_y {
            return None;
        }

        let y = self.triples.wavelet_y.access(self.pos_y).unwrap() as Id;

        if self.search_z > 0 {
            self.pos_y += 1;
            match self.triples.adjlist_z.search(self.pos_y - 1, self.search_z) {
                Some(_) => {
                    return Some(self.triples.coord_to_triple(self.x, y, self.search_z).unwrap());
                }
                None => {
                    return self.next();
                }
            }
        }

        if self.pos_z >= self.max_z {
            return None;
        }
        let z = self.triples.adjlist_z.get_id(self.pos_z);
        let triple_id = self.triples.coord_to_triple(self.x, y, z).unwrap();

        // theoretically the second condition should only be true if the first is as well but in practise it wasn't, which screwed up the subject identifiers
        // fixed by moving the second condition inside the first one but there may be another reason for the bug occuring in the first place
        if self.triples.adjlist_z.at_last_sibling(self.pos_z) {
            if self.triples.bitmap_y.at_last_sibling(self.pos_y) {
                self.x += 1;
            }
            self.pos_y += 1;
        }
        self.pos_z += 1;
        Some(triple_id)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.search_z {
            // exact for VVV, SVV, SPV, SPO
            0 => (self.max_z - self.pos_z, Some(self.max_z - self.pos_z)),
            _ => (0, Some(self.max_z- self.pos_z)), // not exact for SVO
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::{Hdt, HdtGraph, IdKind};
    use sophia::api::graph::Graph;
    use sophia::api::prelude::Any;
    use std::time::Instant;

    #[test]
    fn skip_triples_old_school() {
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
        let graph = HdtGraph::new(hdt);
        let majors = graph.triples_matching(Any,Any,Any);

        println!("{:?}", majors.size_hint());
        let start = Instant::now();
        let mut skipped = majors.skip(5_000_000);
        println!("{:?}", skipped.next());
        println!("{:?}", start.elapsed());

        let start_count = Instant::now();
        println!("{}", skipped.count());
        println!("{:?}", start_count.elapsed());
    }

    #[test]
    fn skip_triples_on_ids() {
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
        let majors = hdt.triple_ids_with_pattern_and_offset(None, None, None, None);

        println!("{:?}", majors.size_hint());
        let start = Instant::now();
        let mut skipped = majors.skip(5_000_000);
        println!("{:?}", skipped.next());
        println!("{:?}", start.elapsed());

        let start_count = Instant::now();
        println!("{}", skipped.count());
        println!("{:?}", start_count.elapsed());
    }

    #[test]
    fn efficient_skip_triples() { // VVV
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
        // let graph = HdtGraph::new(hdt);

        let start = Instant::now();
        let mut majors = hdt.triple_ids_with_pattern_and_offset(None, None, None, Some(5_000_000));
        println!("{:?}", majors.next());
        println!("{:?}", start.elapsed());

        let start_count = Instant::now();
        println!("{}", majors.count());
        println!("{:?}", start_count.elapsed());
    }

    #[test]
    fn skip_on_subject_iterators() {
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s = "http://db.uwaterloo.ca/~galuc/wsdbm/User44276".into();
        let p = "http://db.uwaterloo.ca/~galuc/wsdbm/friendOf".into();
        let o = "http://db.uwaterloo.ca/~galuc/wsdbm/User69629".into();

        let sid = Some(hdt.dict.string_to_id(s, &IdKind::Subject));
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));
        let oid = Some(hdt.dict.string_to_id(o, &IdKind::Object));

        // VVV
        let count_vvv = hdt.triple_ids_with_pattern_and_offset(None, None, None, None);
        println!("vvv  estim: {:?}  vs total : {}", count_vvv.size_hint(), count_vvv.count());
        let skip_vvv = hdt.triple_ids_with_pattern_and_offset(None, None, None, Some(5_000_000));
        println!("skip estim: {:?}  vs actual: {}\n", skip_vvv.size_hint(), skip_vvv.count());

        // SVV
        let count_svv = hdt.triple_ids_with_pattern_and_offset(sid, None, None, None);
        println!("svv  estim: {:?} vs total : {}", count_svv.size_hint(), count_svv.count());
        let skip_svv = hdt.triple_ids_with_pattern_and_offset(sid, None, None, Some(250));
        println!("skip estim: {:?} vs actual: {} \n", skip_svv.size_hint(), skip_svv.count());

        // SPV
        let count_spv = hdt.triple_ids_with_pattern_and_offset(sid, pid, None, None);
        println!("spv estim : {:?} vs total : {}", count_spv.size_hint(), count_spv.count());
        let skip_spv = hdt.triple_ids_with_pattern_and_offset(sid, pid, None, Some(150));
        println!("skip estim: {:?} vs actual: {}\n", skip_spv.size_hint(), skip_spv.count());

        // SVO
        let count_svo = hdt.triple_ids_with_pattern_and_offset(sid, None, oid, None);
        println!("svo estim : {:?} vs total : {}", count_svo.size_hint(), count_svo.count());
        let skip_svo = hdt.triple_ids_with_pattern_and_offset(sid, None, oid, Some(1));
        println!("skip estim: {:?} vs actual: {}\n", skip_svo.size_hint(), skip_svo.count());

        // SPO
        let count_spo = hdt.triple_ids_with_pattern_and_offset(sid, pid, oid, None);
        println!("spo estim : {:?} vs total : {}", count_spo.size_hint(), count_spo.count());
        let skip_spo = hdt.triple_ids_with_pattern_and_offset(sid, pid, oid, Some(1));
        println!("skip estim: {:?} vs actual: {}", skip_spo.size_hint(), skip_spo.count());

    }

    #[test]
    fn skip_over_the_max () {
        todo!()
    }
}