use crate::triples::Id;
use crate::triples::TripleId;
use crate::triples::TriplesBitmap;
use sucds::int_vectors::Access;
// see "Exchange and Consumption of Huge RDF Data" by Martinez et al. 2012
// https://link.springer.com/chapter/10.1007/978-3-642-30284-8_36
// actually only an object iterator when SPO order is used
// TODO test with other orders and fix if broken

/// Iterator over all triples with a given object ID, answering an (?S,?P,O) query.
pub struct ObjectIter<'a> {
    triples: &'a TriplesBitmap,
    o: Id,
    pos_index: usize,
    max_index: usize,
}

impl<'a> ObjectIter<'a> {
    /// Create a new iterator over all triples with the given object ID.
    /// Panics if the object does not exist.
    pub fn new(triples: &'a TriplesBitmap, o: Id) -> Self {
        assert!(o != 0, "object 0 does not exist, cant iterate");
        let pos_index = triples.op_index.find(o);
        let max_index = triples.op_index.last(o);
        //println!("ObjectIter o={} pos_index={} max_index={}", o, pos_index, max_index);
        ObjectIter { triples, o, pos_index, max_index }
    }

    /// Identical to ::new but with an optional offset to quickly skip
    /// to designated offset.
    pub fn new_with_offset(triples: &'a TriplesBitmap, o: Id, op_offset: Option<usize>) -> Self {
        match op_offset {
            None => ObjectIter::new(triples, o),
            Some(offset) => {
                let mut base = ObjectIter::new(triples, o);
                base.pos_index += offset;
                base
            }
        }

    }
}

impl Iterator for ObjectIter<'_> {
    type Item = TripleId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos_index > self.max_index {
            return None;
        }
        let pos_y = self.triples.op_index.sequence.access(self.pos_index).unwrap();
        let y = self.triples.wavelet_y.access(pos_y).unwrap() as Id;
        let x = self.triples.adjlist_y.bitmap.rank(pos_y) as Id + 1;
        self.pos_index += 1;
        Some(TripleId::new(x, y, self.o))
        //Some(self.triples.coord_to_triple(x, y, self.o).unwrap())
    }

    /// Provides exact cardinality on VVO.
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.pos_index > self.max_index {(0, Some(0))}
        else {(self.max_index - self.pos_index + 1, Some(self.max_index - self.pos_index + 1))}
    }

}


#[cfg(test)]
mod tests {
    use crate::{Hdt, IdKind};

    #[test]
    fn skip_on_vvo() {
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s = "http://db.uwaterloo.ca/~galuc/wsdbm/User44276".into();
        let p = "http://db.uwaterloo.ca/~galuc/wsdbm/friendOf".into();
        let o = "http://db.uwaterloo.ca/~galuc/wsdbm/User69629".into();

        let sid = Some(hdt.dict.string_to_id(s, &IdKind::Subject));
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));
        let oid = Some(hdt.dict.string_to_id(o, &IdKind::Object));

        // VVO
        let count_vvo = hdt.triple_ids_with_pattern_and_offset(None, None, oid, None);
        println!("vvo  estim: {:?}  vs total : {}", count_vvo.size_hint(), count_vvo.count());
        let skip_vvo = hdt.triple_ids_with_pattern_and_offset(None, None, oid, Some(20));
        println!("skip estim: {:?}  vs actual: {}\n", skip_vvo.size_hint(), skip_vvo.count());
    }
}