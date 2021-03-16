//! # startin
//!
//! [![crates.io](https://img.shields.io/crates/v/startin.svg)](https://crates.io/crates/startin)
//!
//! A Delaunay triangulator where the input are 2.5D points, the DT is computed in 2D but the elevation of the vertices are kept.
//! This is used mostly for the modelling of terrains.
//!
//! The construction algorithm used is an incremental insertion based on flips, and the data structure is a cheap implementation of the star-based structure defined in [Blandford et al. (2003)](https://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.9.6823), cheap because the link of each vertex is stored a simple array (`Vec`) and not in an optimised blob like they did.
//! It results in a pretty fast library (comparison will come at some point), but it uses more space than the optimised one.
//!
//! The deletion of a vertex is also possible. The algorithm implemented is a modification of the one of [Mostafavi, Gold, and Dakowicz (2003)](https://doi.org/10.1016/S0098-3004(03)00017-7). The ears are filled by flipping, so it's in theory more robust. I have also extended the algorithm to allow the deletion of vertices on the boundary of the convex hull. The algorithm is sub-optimal, but in practice the number of neighbours of a given vertex in a DT is only 6, so it doesn't really matter.
//!
//! Robust arithmetic for the geometric predicates are used ([Shewchuk's predicates](https://www.cs.cmu.edu/~quake/robust.html), well its [Rust port](https://github.com/Stoeoef/spade/blob/master/src/exactpred.rs)), so the library is robust and shouldn't crash (touch wood).
//!
//! I made this in Rust because I wanted to learn Rust.
//!
//! # Python bindings
//!
//! If you prefer Python, I made bindings: [https://github.com/hugoledoux/startin_python/](https://github.com/hugoledoux/startin_python/)
//!
//! # Web-demo with WebAssembly
//!
//! Rust can be compiled easily to [WebAssembly](https://www.rust-lang.org/what/wasm), and you see a demo of the possibilities of startin (all computations are done locally and it's fast!).
//!
//! [web-demo](https://hugoledoux.github.io/startin_wasm/www/dist/)
//!
//! # Usage
//!
//! ```rust
//! extern crate startin;
//!
//! fn main() {
//!     let mut pts: Vec<Vec<f64>> = Vec::new();
//!     pts.push(vec![20.0, 30.0, 2.0]);
//!     pts.push(vec![120.0, 33.0, 12.5]);
//!     pts.push(vec![124.0, 222.0, 7.65]);
//!     pts.push(vec![20.0, 133.0, 21.0]);
//!     pts.push(vec![60.0, 60.0, 33.0]);
//!
//!     let mut dt = startin::Triangulation::new();
//!     dt.insert(&pts);
//!
//!     println!("*****");
//!     println!("Number of points in DT: {}", dt.number_of_vertices());
//!     println!("Number of triangles in DT: {}", dt.number_of_triangles());
//!
//!     //-- print all the vertices
//!     for (i, each) in dt.all_vertices().iter().enumerate() {
//!         // skip the first one, the infinite vertex
//!         if i > 0 {
//!             println!("#{}: ({:.3}, {:.3}, {:.3})", i, each[0], each[1], each[2]);
//!         }
//!     }
//!
//!     //-- insert a new vertex
//!     let re = dt.insert_one_pt(22.2, 33.3, 4.4, 0);
//!     match re {
//!         Ok(_v) => println!("Inserted new point"),
//!         Err(v) => println!("Duplicate of vertex #{}, not inserted", v),
//!     }
//!     //-- remove it
//!     let re = dt.remove(6);
//!     if re.is_err() == true {
//!         println!("!!! Deletion error: {:?}", re.unwrap_err());
//!     } else {
//!         println!("Deleted vertex");
//!     }
//!
//!     //-- get the convex hull
//!     let ch = dt.convex_hull();
//!     println!("Convex hull: {:?}", ch);
//!
//!     //-- fetch triangle containing (x, y)
//!     let re = dt.locate(50.0, 50.0);
//!     if re.is_some() {
//!         let t = re.unwrap();
//!         println!("The triangle is {}", t);
//!         assert!(dt.is_triangle(&t));
//!     } else {
//!         println!("Outside convex hull");
//!     }
//!
//!     //-- some stats
//!     println!("Number of points in DT: {}", dt.number_of_vertices());
//!     println!("Number of triangles in DT: {}", dt.number_of_triangles());
//! }
//! ```

mod geom;

#[cfg(feature = "c_api")]
mod c_interface;

use rand::prelude::thread_rng;
use rand::Rng;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::io::{self};

use geojson::{Feature, FeatureCollection, Geometry, Value};
use serde_json::{to_value, Map};

use hashbrown::HashMap;

extern crate rand;

#[macro_use]
extern crate log; //info/debug/error

/// A Triangle is a triplet of indices
pub struct Triangle {
    pub v: [usize; 3],
}

impl Triangle {
    /// Checks whether a Triangle is "infinite",
    /// ie if one its vertices is the infinite vertex
    fn is_infinite(&self) -> bool {
        if self.v[0] == 0 || self.v[1] == 0 || self.v[2] == 0 {
            return true;
        }
        return false;
    }
}

impl fmt::Display for Triangle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.v[0], self.v[1], self.v[2])
    }
}

//----------------------
#[repr(C)]
struct Link(Vec<usize>);

impl Link {
    fn new() -> Link {
        // Link(Vec::new())
        Link(Vec::with_capacity(8))
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn is_empty(&self) -> bool {
        if self.0.len() == 0 {
            true
        } else {
            false
        }
    }
    fn add(&mut self, v: usize) {
        self.0.push(v);
    }
    fn insert_after_v(&mut self, v: usize, after: usize) {
        let pos = self.0.iter().position(|&x| x == after);
        if pos != None {
            self.0.insert(pos.unwrap() + 1, v);
        }
    }
    fn delete(&mut self, v: usize) {
        let re = self.0.iter().position(|&x| x == v);
        if re != None {
            self.0.remove(re.unwrap());
        }
    }
    fn replace(&mut self, v: usize, newv: usize) {
        let re = self.0.iter().position(|&x| x == v);
        if re != None {
            self.0[re.unwrap()] = newv;
            // self.0.remove(re.unwrap());
        }
    }
    fn infinite_first(&mut self) {
        let re = self.0.iter().position(|&x| x == 0);
        if re != None {
            let posinf = re.unwrap();
            if posinf == 0 {
                return;
            }
            let mut newstar: Vec<usize> = Vec::new();
            for j in posinf..self.0.len() {
                newstar.push(self.0[j]);
            }
            for j in 0..posinf {
                newstar.push(self.0[j]);
            }
            // println!("newstar: {:?}", newstar);
            self.0 = newstar;
        }
    }
    fn clear(&mut self) {
        self.0.clear();
    }

    fn contains_infinite_vertex(&self) -> bool {
        let pos = self.0.iter().position(|&x| x == 0);
        if pos == None {
            return false;
        } else {
            return true;
        }
    }

    fn next_index(&self, i: usize) -> usize {
        if i == (self.0.len() - 1) {
            0
        } else {
            i + 1
        }
    }

    fn prev_index(&self, i: usize) -> usize {
        if i == 0 {
            self.0.len() - 1
        } else {
            i - 1
        }
    }

    fn get_index(&self, v: usize) -> Option<usize> {
        return self.0.iter().position(|&x| x == v);
    }

    fn has_index(&self, v: usize) -> bool {
        return self.0.iter().any(|&x| x == v);
    }

    fn get_next_vertex(&self, v: usize) -> Option<usize> {
        let re = self.get_index(v);
        if re.is_none() {
            return None;
        }
        let pos = re.unwrap();
        if pos == (self.0.len() - 1) {
            return Some(self.0[0]);
        } else {
            return Some(self.0[(pos + 1)]);
        }
    }

    fn get_prev_vertex(&self, v: usize) -> Option<usize> {
        let re = self.get_index(v);
        if re.is_none() {
            return None;
        }
        let pos = re.unwrap();
        if pos == 0 {
            return Some(self.0[(self.0.len() - 1)]);
        } else {
            return Some(self.0[(pos - 1)]);
        }
    }

    fn iter(&self) -> Iter {
        Iter(Box::new(self.0.iter()))
    }
}

//-- taken from https://stackoverflow.com/questions/40668074/am-i-incorrectly-implementing-intoiterator-for-a-reference-or-is-this-a-rust-bug
struct Iter<'a>(Box<dyn Iterator<Item = &'a usize> + 'a>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl std::ops::Index<usize> for Link {
    type Output = usize;
    fn index(&self, idx: usize) -> &usize {
        &self.0[idx as usize]
    }
}

impl fmt::Display for Link {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        // fmt.write_str("pt: {}\n", self.pt)?;
        fmt.write_str(&format!("link: {:?}", self.0))?;
        Ok(())
    }
}

/// A triangulation is a collection of Stars, each Star has its (x,y,z)
/// and a Link (an array of adjacent vertices, ordered CCW)
#[repr(C)]
struct Star {
    pub pt: [f64; 3],
    pub link: Link,
    pub written: bool,
    pub local_id: usize,
}

impl Star {
    pub fn new(x: f64, y: f64, z: f64) -> Star {
        let l = Link::new();
        Star {
            pt: [x, y, z],
            link: l,
            written: true,
            local_id: 0,
        }
    }
    // pub fn is_deleted(&self) -> bool {
    //     self.link.is_empty()
    // }
}

//----------------------
#[repr(C)]
pub struct Triangulation {
    stars: HashMap<usize, Star>,
    snaptol: f64,
    cur: usize,
    is_init: bool,
    jump_and_walk: bool,
    robust_predicates: bool,
    theid: usize,
    global_id: usize,
}

impl Triangulation {
    pub fn new() -> Triangulation {
        // TODO: allocate a certain number?

        let mut s: HashMap<usize, Star> = HashMap::new();
        let mystar = Star::new(-99999.99999, -99999.99999, -99999.99999);
        s.insert(0, mystar);

        Triangulation {
            stars: s,
            snaptol: 0.001,
            cur: 0,
            is_init: false,
            jump_and_walk: false,
            robust_predicates: true,
            theid: 1,
            global_id: 1,
        }
    }

    fn insert_one_pt_init_phase(&mut self, x: f64, y: f64, z: f64) -> Result<usize, usize> {
        let p: [f64; 3] = [x, y, z];
        for i in 1..self.stars.len() {
            if geom::distance2d_squared(&self.stars[&i].pt, &p) <= (self.snaptol * self.snaptol) {
                return Err(i);
            }
        }
        //-- add point to Triangulation and create its empty star
        self.stars.insert(self.theid, Star::new(x, y, z));
        self.theid += 1;
        //-- form the first triangles (finite + infinite)
        let l = self.stars.len();
        if l >= 4 {
            let a = l - 3;
            let b = l - 2;
            let c = l - 1;
            let re = geom::orient2d(
                &self.stars[&a].pt,
                &self.stars[&b].pt,
                &self.stars[&c].pt,
                self.robust_predicates,
            );
            if re == 1 {
                // println!("init: ({},{},{})", a, b, c);
                self.stars.get_mut(&0).unwrap().link.add(a);
                self.stars.get_mut(&0).unwrap().link.add(c);
                self.stars.get_mut(&0).unwrap().link.add(b);
                self.stars.get_mut(&a).unwrap().link.add(0);
                self.stars.get_mut(&a).unwrap().link.add(b);
                self.stars.get_mut(&a).unwrap().link.add(c);
                self.stars.get_mut(&b).unwrap().link.add(0);
                self.stars.get_mut(&b).unwrap().link.add(c);
                self.stars.get_mut(&b).unwrap().link.add(a);
                self.stars.get_mut(&c).unwrap().link.add(0);
                self.stars.get_mut(&c).unwrap().link.add(a);
                self.stars.get_mut(&c).unwrap().link.add(b);
                self.is_init = true;
            } else if re == -1 {
                // println!("init: ({},{},{})", a, c, b);
                self.stars.get_mut(&0).unwrap().link.add(a);
                self.stars.get_mut(&0).unwrap().link.add(b);
                self.stars.get_mut(&0).unwrap().link.add(c);
                self.stars.get_mut(&a).unwrap().link.add(0);
                self.stars.get_mut(&a).unwrap().link.add(c);
                self.stars.get_mut(&a).unwrap().link.add(b);
                self.stars.get_mut(&b).unwrap().link.add(0);
                self.stars.get_mut(&b).unwrap().link.add(a);
                self.stars.get_mut(&b).unwrap().link.add(c);
                self.stars.get_mut(&c).unwrap().link.add(0);
                self.stars.get_mut(&c).unwrap().link.add(b);
                self.stars.get_mut(&c).unwrap().link.add(a);
                self.is_init = true;
            }
        }
        self.cur = l - 1;
        if self.is_init == true {
            //-- insert the previous vertices in the dt
            for j in 1..(l - 3) {
                let tr = self.walk(&self.stars[&j].pt);
                // println!("found tr: {}", tr);
                self.flip13(j, &tr);
                self.update_dt(j);
            }
        }
        Ok(self.cur)
    }

    /// Set a snap tolerance when inserting new points: if the newly inserted
    /// one is closer than snap_tolerance to another one, then it is not inserted.
    /// Avoids having very close vertices (like at 0.00007mm)
    /// Default is 0.001unit (thus 1mm for most datasets).
    pub fn set_snap_tolerance(&mut self, snaptol: f64) -> f64 {
        if snaptol > 0.0 {
            self.snaptol = snaptol;
        }
        self.snaptol
    }

    pub fn get_snap_tolerance(&self) -> f64 {
        self.snaptol
    }

    /// Activate/deactive the jump-and-walk strategy for locate().
    /// If deactivated, then the walk starts from the last inserted triangle.
    pub fn set_jump_and_walk(&mut self, b: bool) {
        self.jump_and_walk = b;
    }

    pub fn is_using_robust_predicates(&self) -> bool {
        self.robust_predicates
    }

    pub fn use_robust_predicates(&mut self, b: bool) {
        self.robust_predicates = b;
    }

    // why not use ndarray or similar here?
    pub fn insert(&mut self, pts: &Vec<Vec<f64>>) {
        let mut duplicates = 0;
        for each in pts {
            if (each.len() < 2) || (each.len() > 3) {
                panic!(
                    "Point {:?} should be 2D or 3D (and is now {}D).",
                    each,
                    each.len()
                );
            } else {
                let re;
                if each.len() == 2 {
                    re = self.insert_one_pt(each[0], each[1], 0.0, 0);
                } else {
                    re = self.insert_one_pt(each[0], each[1], each[2], 0);
                }
                match re {
                    Ok(_x) => continue,
                    Err(_e) => duplicates = duplicates + 1,
                }
            }
        }
    }

    //-- insert_one_pt
    pub fn insert_one_pt(
        &mut self,
        px: f64,
        py: f64,
        pz: f64,
        vertex_id: usize,
    ) -> Result<usize, usize> {
        // println!("-->{}", p);
        if !self.is_init {
            return self.insert_one_pt_init_phase(px, py, pz);
        }
        //-- walk
        let p: [f64; 3] = [px, py, pz];
        let tr = self.walk(&p);

        // println!("STARTING TR: {}", tr);
        // println!(
        //     "{:?} \n {:?} \n {:?}",
        //     self.stars[&tr.v[0]].pt.to_vec(),
        //     self.stars[&tr.v[1]].pt.to_vec(),
        //     self.stars[&tr.v[2]].pt.to_vec()
        // );

        if geom::distance2d_squared(&self.stars[&tr.v[0]].pt, &p) <= (self.snaptol * self.snaptol) {
            return Err(tr.v[0]);
        }
        if geom::distance2d_squared(&self.stars[&tr.v[1]].pt, &p) <= (self.snaptol * self.snaptol) {
            return Err(tr.v[1]);
        }
        if geom::distance2d_squared(&self.stars[&tr.v[2]].pt, &p) <= (self.snaptol * self.snaptol) {
            return Err(tr.v[2]);
        }
        let mut pi: usize = self.theid;

        //-- ok we now insert the point in the data structure
        //-- TODO: remove this for delete when hash is used

        if vertex_id != 0 {
            pi = vertex_id;
            self.stars.insert(vertex_id, Star::new(px, py, pz));
        } else {
            self.stars.insert(self.theid, Star::new(px, py, pz));
            self.theid += 1;
        }

        //-- flip13()
        self.flip13(pi, &tr);

        //-- update_dt()
        self.update_dt(pi);

        self.cur = pi;
        Ok(pi)
    }

    pub fn insert_vertex(&mut self, vertex_id: usize, px: f64, py: f64, pz: f64) {
        self.stars.insert(vertex_id, Star::new(px, py, pz));
        // self.stars.push(Star::new(px, py, pz));
        // self.cur = self.stars.len() - 1;
        self.theid = vertex_id;
    }

    pub fn define_star(&mut self, center_vertex_id: usize, neighbors: Vec<usize>) {
        // // Clear the link so the neighbors are ensured in the right order
        // self.stars.get_mut(&center_vertex_id).unwrap().link.clear();

        for neighbor in neighbors {
            self.stars
                .get_mut(&center_vertex_id)
                .unwrap()
                .link
                .add(neighbor);

            // // Add this vertex to the neighbors too
            // // !! Might be in wrong order !!
            // if !self.stars[&neighbor].link.has_index(center_vertex_id) {
            //     self.stars.get_mut(&neighbor).unwrap().link.add(center_vertex_id);
            // }
        }

        self.cur = center_vertex_id;
    }

    pub fn set_is_init(&mut self, is_init: bool) {
        self.is_init = is_init;
    }

    fn update_dt(&mut self, pi: usize) {
        // println!("--> Update DT");
        let mut mystack: Vec<Triangle> = Vec::new();
        let l = &self.stars.get_mut(&pi).unwrap().link;
        mystack.push(Triangle {
            v: [pi, l[0], l[1]],
        });
        mystack.push(Triangle {
            v: [pi, l[1], l[2]],
        });
        mystack.push(Triangle {
            v: [pi, l[2], l[0]],
        });

        loop {
            let tr = match mystack.pop() {
                None => break,
                Some(x) => x,
            };
            // println!("tr: {}", tr);
            let opposite = self.get_opposite_vertex(&tr);
            //-- TODO: danger danger
            if !self.stars.contains_key(&opposite) {
                continue;
            }
            //----------------------
            // println!("stacked: {} {}", tr, opposite);

            if tr.is_infinite() {
                let mut a: i8 = 0;
                if tr.v[0] == 0 {
                    a = geom::orient2d(
                        &self.stars[&opposite].pt,
                        &self.stars[&tr.v[1]].pt,
                        &self.stars[&tr.v[2]].pt,
                        self.robust_predicates,
                    );
                } else if tr.v[1] == 0 {
                    a = geom::orient2d(
                        &self.stars[&tr.v[0]].pt,
                        &self.stars[&opposite].pt,
                        &self.stars[&tr.v[2]].pt,
                        self.robust_predicates,
                    );
                } else if tr.v[2] == 0 {
                    a = geom::orient2d(
                        &self.stars[&tr.v[0]].pt,
                        &self.stars[&tr.v[1]].pt,
                        &self.stars[&opposite].pt,
                        self.robust_predicates,
                    );
                }
                // println!("TODO: INCIRCLE FOR INFINITY {}", a);
                if a > 0 {
                    // println!("FLIPPED0 {} {}", tr, opposite);
                    let (ret0, ret1) = self.flip22(&tr, opposite);
                    mystack.push(ret0);
                    mystack.push(ret1);
                }
            } else {
                //- if insertion on CH then break the edge, otherwise do nothing
                //-- TODO sure the flips are okay here?
                if opposite == 0
                    && geom::orient2d(
                        &self.stars[&tr.v[0]].pt,
                        &self.stars[&tr.v[1]].pt,
                        &self.stars[&tr.v[2]].pt,
                        self.robust_predicates,
                    ) == 0
                {
                    // println!("FLIPPED1 {} {}", tr, 0);
                    let (ret0, ret1) = self.flip22(&tr, 0);

                    mystack.push(ret0);
                    mystack.push(ret1);
                } else if geom::incircle(
                    &self.stars[&tr.v[0]].pt,
                    &self.stars[&tr.v[1]].pt,
                    &self.stars[&tr.v[2]].pt,
                    &self.stars[&opposite].pt,
                    self.robust_predicates,
                ) > 0
                {
                    // println!("FLIPPED2 {} {}", tr, opposite);
                    let (ret0, ret1) = self.flip22(&tr, opposite);
                    mystack.push(ret0);
                    mystack.push(ret1);
                }
            }
        }
    }

    fn flip13(&mut self, pi: usize, tr: &Triangle) {
        let l = &mut self.stars.get_mut(&pi).unwrap().link;
        l.add(tr.v[0]);
        l.add(tr.v[1]);
        l.add(tr.v[2]);
        self.stars
            .get_mut(&tr.v[0])
            .unwrap()
            .link
            .insert_after_v(pi, tr.v[1]);
        self.stars
            .get_mut(&tr.v[1])
            .unwrap()
            .link
            .insert_after_v(pi, tr.v[2]);
        self.stars
            .get_mut(&tr.v[2])
            .unwrap()
            .link
            .insert_after_v(pi, tr.v[0]);
        //-- put infinite vertex first in list
        // self.stars[pi].link.infinite_first();
    }

    fn flip31(&mut self, v: usize) {
        // println!("FLIP31");
        let mut ns: Vec<usize> = Vec::new();
        for each in self.stars[&v].link.iter() {
            ns.push(*each);
        }
        for n in ns.iter() {
            self.stars.get_mut(&n).unwrap().link.delete(v);
        }
        self.stars.get_mut(&v).unwrap().link.clear();
        self.stars.remove(&v);
        // self.removed_indices.push(v);
        // println!("ns length = {}", ns.len());
        // Set self.cur to star we know is finalized
        if ns.len() == 0 {
        } else if ns[0] != 0 && !self.stars[&ns[0]].link.is_empty() {
            self.cur = ns[0];
        } else if ns[1] != 0 && !self.stars[&ns[1]].link.is_empty() {
            self.cur = ns[1];
        } else if ns[2] != 0 && !self.stars[&ns[2]].link.is_empty() {
            self.cur = ns[2];
        }
    }

    /// Returns the coordinates of the vertex v in a Vec [x,y,z]
    pub fn get_point(&self, v: usize) -> Option<Vec<f64>> {
        if !self.vertex_exists(v) {
            None
        } else {
            Some(self.stars[&v].pt.to_vec())
        }
    }

    pub fn adjacent_triangles_to_triangle(&self, tr: &Triangle) -> Option<Vec<Triangle>> {
        if self.is_triangle(&tr) == false || tr.is_infinite() == true {
            return None;
        }
        let mut trs: Vec<Triangle> = Vec::new();
        let mut opp = self.stars[&tr.v[2]].link.get_next_vertex(tr.v[1]).unwrap();
        if opp != 0 {
            trs.push(Triangle {
                v: [tr.v[1], opp, tr.v[2]],
            });
        }
        opp = self.stars[&tr.v[0]].link.get_next_vertex(tr.v[2]).unwrap();
        if opp != 0 {
            trs.push(Triangle {
                v: [tr.v[2], opp, tr.v[0]],
            });
        }
        opp = self.stars[&tr.v[1]].link.get_next_vertex(tr.v[0]).unwrap();
        if opp != 0 {
            trs.push(Triangle {
                v: [tr.v[0], opp, tr.v[1]],
            });
        }
        Some(trs)
    }

    /// Returns a Vec of Triangles (finite + infinite) to the vertex v.
    /// If v doesn't exist, then [`None`] is returned.
    pub fn incident_triangles_to_vertex(&self, v: usize) -> Option<Vec<Triangle>> {
        if self.vertex_exists(v) == false {
            return None;
        }
        let mut trs: Vec<Triangle> = Vec::new();
        for (i, each) in self.stars[&v].link.iter().enumerate() {
            let j = self.stars[&v].link.next_index(i);
            trs.push(Triangle {
                v: [v, *each, self.stars[&v].link[j]],
            });
        }
        Some(trs)
    }

    /// Returns the degree of a vertex, [`None`] is it doesn't exist.
    pub fn degree(&self, v: usize) -> Option<usize> {
        if self.vertex_exists(v) == false {
            return None;
        }
        Some(self.stars[&v].link.len())
    }

    /// Returns a list (`Vec<usize>`) (ordered CCW) of the adjacent vertices.
    /// [`None`] if the vertex is not part of the triangulation.
    pub fn adjacent_vertices_to_vertex(&self, v: usize) -> Option<Vec<usize>> {
        if self.vertex_exists(v) == false {
            return None;
        }
        let mut adjs: Vec<usize> = Vec::new();
        for each in self.stars[&v].link.iter() {
            adjs.push(*each);
        }
        Some(adjs)
    }

    /// Returns whether a triplet of indices is a Triangle in the triangulation.
    pub fn is_triangle(&self, tr: &Triangle) -> bool {
        // TODO: what about infinite triangles?
        let re = self.stars[&tr.v[0]].link.get_next_vertex(tr.v[1]);
        if re.is_none() {
            return false;
        } else {
            if re.unwrap() == tr.v[2] {
                return true;
            } else {
                return false;
            }
        }
    }

    pub fn statistics_degree(&self) -> (f64, usize, usize) {
        let mut total: f64 = 0.0;
        let mut min: usize = usize::max_value();
        let mut max: usize = usize::min_value();
        for i in 1..self.stars.len() {
            total = total + self.stars[&i].link.len() as f64;
            if self.stars[&i].link.len() > max {
                max = self.stars[&i].link.len();
            }
            if self.stars[&i].link.len() < min {
                min = self.stars[&i].link.len();
            }
        }
        total = total / (self.stars.len() - 2) as f64;
        return (total, min, max);
    }

    /// Returns number of finite vertices in the triangulation.
    pub fn number_of_vertices(&self) -> usize {
        //-- number of finite vertices
        self.stars.len() - 1
    }

    pub fn cleanup_complete_stars(&mut self) {
        let mut to_remove: Vec<usize> = Vec::new();

        for vertex_id in self.all_vertex_ids() {
            if !self.stars[&vertex_id].written
                || self.stars[&vertex_id].local_id == 0
                || !self.is_star_loaded(vertex_id)
            {
                continue;
            }

            // If one of the neighbors isn't written, we can't remove this vertex yet
            // as we might need this vertex in the process of a walk later on
            for neighbor in self.stars[&vertex_id].link.iter() {
                if !self.vertex_exists(*neighbor) || !self.stars[&neighbor].written {
                    continue;
                }
            }

            to_remove.push(vertex_id);
        }

        // println!("Removed {} vertices in cleanup!", to_remove.len());

        for vertex in to_remove {
            self.stars.remove(&vertex);
        }
    }

    /// Returns number of finite triangles in the triangulation.
    pub fn number_of_triangles(&self) -> usize {
        //-- number of finite triangles
        let mut count: usize = 0;
        for (i, star) in &self.stars {
            for (j, value) in star.link.iter().enumerate() {
                if i < value {
                    let k = star.link[star.link.next_index(j)];
                    if i < &k {
                        let tr = Triangle { v: [*i, *value, k] };
                        if tr.is_infinite() == false {
                            count = count + 1;
                        }
                    }
                }
            }
        }
        count
    }

    // /// Returns the number of vertices which are marked as "removed"
    // pub fn number_of_removed_vertices(&self) -> usize {
    //     self.removed_indices.len()
    // }

    pub fn is_vertex_removed(&self, v: usize) -> bool {
        !self.vertex_exists(v)
    }

    pub fn can_vertex_be_removed(&self, v: usize) -> bool {
        // Vertex exists, is not on the convex hull, and is also not the infinite vertex
        if !self.vertex_exists(v) || self.is_vertex_convex_hull(v) || v == 0 {
            return false;
        }
        true
    }

    /// Returns the convex hull of the dataset, oriented CCW.
    /// It is a list of vertex indices (first != last)
    pub fn convex_hull(&self) -> Vec<usize> {
        let mut re: Vec<usize> = Vec::new();
        for x in self.stars[&0].link.iter() {
            re.push(*x);
        }
        re.reverse();
        re
    }

    /// Returns the size (ie the number of vertices) of the convex hull of the dataset
    pub fn number_of_vertices_on_convex_hull(&self) -> usize {
        //-- number of finite vertices on the boundary of the convex hull
        if self.is_init == false {
            return 0;
        }
        return self.stars[&0].link.len();
    }

    pub fn is_star_loaded(&self, v: usize) -> bool {
        let mut is_loaded: bool = true;
        for neighboring_vertex in self.stars[&v].link.iter() {
            if !self.vertex_exists(*neighboring_vertex)
                || self.stars[&neighboring_vertex].link.is_empty()
            {
                is_loaded = false;
                break;
            }
        }
        is_loaded
    }

    /// Returns true if the vertex v is part of the boundary of the convex
    /// hull of the dataset. False otherwise.
    pub fn is_vertex_convex_hull(&self, v: usize) -> bool {
        // If the star isn't entirely loaded, it's technically on the CH
        if self.stars[&v].link.contains_infinite_vertex()
            || self.stars[&v].link.is_empty()
            || !self.is_star_loaded(v)
        {
            return true;
        }
        return false;
    }

    /// Returns, if it exists, the Triangle containing (px,py).
    /// If it is direction on a vertex/edge, then one is randomly chosen.
    pub fn locate(&self, px: f64, py: f64) -> Option<Triangle> {
        let p: [f64; 3] = [px, py, 0.0];
        let re = self.walk(&p);
        match re.is_infinite() {
            true => None,
            false => Some(re),
        }
    }

    // Returns closest point (in 2D) to a query point (x,y).
    // if (x,y) is outside the convex hull [`None`]
    pub fn closest_point(&self, px: f64, py: f64) -> Option<usize> {
        let re = self.locate(px, py);
        if re.is_none() == true {
            return None;
        }
        let p: [f64; 3] = [px, py, 0.0];
        let tr = re.unwrap();
        let mut d = std::f64::MAX;
        let mut closest: usize = 0;
        //-- 1. find triangle and closest vertex from the 3
        for each in tr.v.iter() {
            // println!("{}", each);
            let dtmp = geom::distance2d_squared(&self.stars[each].pt, &p);
            if dtmp < d {
                d = dtmp;
                closest = *each;
            }
        }
        for each in self.stars[&closest].link.iter() {
            let dtmp = geom::distance2d_squared(&self.stars[each].pt, &p);
            if dtmp < d {
                d = dtmp;
                closest = *each;
            }
        }
        Some(closest)
    }

    fn walk(&self, x: &[f64]) -> Triangle {
        //-- find a starting tr

        //-- 1. try walk from latest
        let re = self.walk_safe(x, self.cur);
        if re.is_some() {
            return re.unwrap();
        }

        //-- 2. try walk from one in the same cell
        // warn!("attempt to find one vertex in the grid cell and start from it");
        // let g = self.qt.get_cell_gxgy(x[0], x[1]);
        // if self.qt.gpts[g.0][g.1].len() > 0 {
        //     let mut dmin: f64 = std::f64::MAX;
        //     let mut vmin: usize = 0;
        //     for i in &self.qt.gpts[g.0][g.1] {
        //         if *i != 0 {
        //             let d = geom::distance2d_squared(x, &self.stars.get(&i).unwrap().pt);
        //             if d < dmin {
        //                 dmin = d;
        //                 vmin = *i;
        //             }
        //         }
        //     }
        //     // cur = *self.qt.gpts[g.0][g.1].iter().next().unwrap();
        //     let re = self.walk_safe(x, vmin);
        //     if re.is_some() {
        //         return re.unwrap();
        //     }
        // }

        //-- 3. try brute-force
        let re2 = self.walk_bruteforce_closest_vertex_then_walksafe(x);
        if re2.is_some() {
            return re2.unwrap();
        }

        let re3 = self.walk_bruteforce_triangles(x);
        if re3.is_some() {
            return re3.unwrap();
        }

        //-- 4. we are outside the CH of the current dataset
        // warn!("point is outside the CH, finding closest point on the CH");
        let re4 = self.walk_bruteforce_vertices(x);
        // for key in self.stars.keys() {
        //     println!("{:?}", key);
        // }
        if re4.is_some() {
            return re4.unwrap();
        } else {
            error!("WALK FAILED MISERABLY :'(");
        }

        let tr = Triangle { v: [0, 0, 0] };
        return tr;
    }

    fn walk_safe(&self, x: &[f64], cur: usize) -> Option<Triangle> {
        // println!("cur: {}, {:?}, {}", cur, self.stars[&cur].pt.to_vec(), self.stars[&cur].link);

        let mut tr = Triangle { v: [0, 0, 0] };
        let re = &self.stars.get(&cur);
        if re.is_none() {
            println!("Apparently current star is None");
            return None;
        }
        if re.unwrap().link.is_empty() {
            println!("Apparently current star has 0 links...");
            return None;
        }

        //--
        // println!(
        //     "walk_safe: ({}) -- {}",
        //     cur,
        //     self.stars.get(&cur).unwrap().link
        // );
        // let a: Vec<&usize> = self.stars.keys().collect();
        // println!("{:?}", a);
        //--

        tr.v[0] = cur;
        let l = &re.unwrap().link;
        let mut b = false;

        for i in 0..(l.len() - 1) {
            if l[i] != 0 && l[i + 1] != 0 {
                tr.v[1] = l[i];
                tr.v[2] = l[i + 1];
                b = true;
                break;
            }
        }

        if !b {
            info!("Cannot find a starting finite triangle.");
            return None;
        }

        if !self.vertex_exists(tr.v[0])
            || !self.vertex_exists(tr.v[1])
            || !self.vertex_exists(tr.v[2])
        {
            return None;
        }

        //-- 2. order it such that tr0-tr1-x is CCW
        if geom::orient2d(
            &self.stars[&tr.v[0]].pt,
            &self.stars[&tr.v[1]].pt,
            &x,
            self.robust_predicates,
        ) == -1
        {
            if geom::orient2d(
                &self.stars[&tr.v[1]].pt,
                &self.stars[&tr.v[2]].pt,
                &x,
                self.robust_predicates,
            ) != -1
            {
                let tmp: usize = tr.v[0];
                tr.v[0] = tr.v[1];
                tr.v[1] = tr.v[2];
                tr.v[2] = tmp;
            } else {
                let tmp: usize = tr.v[1];
                tr.v[1] = tr.v[0];
                tr.v[0] = tr.v[2];
                tr.v[2] = tmp;
            }
        }

        //-- 3. start the walk
        //-- we know that tr0-tr1-x is CCW
        loop {
            // println!("Walk Safe loop: {}", tr);
            // Either current triangle is infinite; or one of the triangles we would want to walk to doesn't exist (yet)
            if tr.is_infinite() {
                break;
            }

            if !self.vertex_exists(tr.v[0]) || self.stars[&tr.v[0]].link.is_empty() {
                return None;
            }
            if !self.vertex_exists(tr.v[1]) || self.stars[&tr.v[1]].link.is_empty() {
                return None;
            }
            if !self.vertex_exists(tr.v[2]) || self.stars[&tr.v[2]].link.is_empty() {
                return None;
            }
            if geom::orient2d(
                &self.stars.get(&tr.v[1]).unwrap().pt,
                &self.stars.get(&tr.v[2]).unwrap().pt,
                &x,
                self.robust_predicates,
            ) != -1
            {
                if geom::orient2d(
                    &self.stars.get(&tr.v[2]).unwrap().pt,
                    &self.stars.get(&tr.v[0]).unwrap().pt,
                    &x,
                    self.robust_predicates,
                ) != -1
                {
                    break;
                } else {
                    //-- walk to incident to tr1,tr2
                    let prev = self
                        .stars
                        .get(&tr.v[2])
                        .unwrap()
                        .link
                        .get_prev_vertex(tr.v[0])
                        .unwrap();
                    tr.v[1] = tr.v[2];
                    tr.v[2] = prev;
                }
            } else {
                //-- walk to incident to tr1,tr2
                // a.iter().position(|&x| x == 2), Some(1)
                let prev = self
                    .stars
                    .get(&tr.v[1])
                    .unwrap()
                    .link
                    .get_prev_vertex(tr.v[2])
                    .unwrap();
                tr.v[0] = tr.v[2];
                tr.v[2] = prev;
            }
        }
        return Some(tr);
    }

    fn walk_bruteforce_vertices(&self, x: &[f64]) -> Option<Triangle> {
        //-- find closest vertex that is on the CH
        let mut dmin: f64 = std::f64::MAX;
        let mut vmin: usize = 0;
        for i in self.stars.keys() {
            if *i != 0 && self.is_vertex_convex_hull(*i) {
                let d = geom::distance2d_squared(x, &self.stars.get(i).unwrap().pt);
                if d < dmin {
                    dmin = d;
                    vmin = *i;
                }
            }
        }

        // info!("brute-force ON CONVEX HULL");
        let mut tr = Triangle { v: [0, 0, 0] };
        let l = &self.stars.get(&vmin).unwrap().link;

        // println!("vmin: {} - {}, {:?}", &vmin, l, self.stars[&vmin].pt.to_vec());

        let mut v2: usize = l.get_prev_vertex(0).unwrap();
        if geom::orient2d(
            &self.stars[&vmin].pt,
            &self.stars[&v2].pt,
            &x,
            self.robust_predicates,
        ) == 1
        {
            tr.v[0] = vmin;
            tr.v[1] = v2;
            tr.v[2] = 0;
        } else {
            v2 = l.get_next_vertex(0).unwrap();
            tr.v[0] = v2;
            tr.v[1] = vmin;
            tr.v[2] = 0;
        }
        return Some(tr);
        // self.walk_safe(x, vmin)
    }

    fn walk_bruteforce_triangles(&self, x: &[f64]) -> Option<Triangle> {
        for (i, star) in &self.stars {
            for (j, value) in star.link.iter().enumerate() {
                if i < value && self.vertex_exists(*value) {
                    let k = star.link[star.link.next_index(j)];

                    if i < &k && self.vertex_exists(k) {
                        let tr = Triangle { v: [*i, *value, k] };

                        if !tr.is_infinite()
                            && geom::intriangle(
                                &self.stars.get(&tr.v[0]).unwrap().pt,
                                &self.stars.get(&tr.v[1]).unwrap().pt,
                                &self.stars.get(&tr.v[2]).unwrap().pt,
                                &x,
                                self.robust_predicates,
                            ) == 1
                        {
                            return Some(tr);
                        }
                    }
                }
            }
        }
        return None;
    }

    fn walk_bruteforce_closest_vertex_then_walksafe(&self, x: &[f64]) -> Option<Triangle> {
        //-- find closest vertex that is on the CH
        let mut dmin: f64 = std::f64::MAX;
        let mut vmin: usize = 0;
        for i in self.stars.keys() {
            // Ensure we find a star that actually has links and is also not infinite
            if !self.stars[i].link.is_empty() && *i != 0 {
                let d = geom::distance2d_squared(x, &self.stars.get(i).unwrap().pt);
                if d < dmin {
                    // println!("Distance from {:?} to {} is {}", x, i, d);
                    dmin = d;
                    vmin = *i;
                }
            }
        }
        // println!("vmin = {}", vmin);
        self.walk_safe(x, vmin)
    }

    fn flip22(&mut self, tr: &Triangle, opposite: usize) -> (Triangle, Triangle) {
        //-- step 1.
        self.stars
            .get_mut(&tr.v[0])
            .unwrap()
            .link
            .insert_after_v(opposite, tr.v[1]);
        //-- step 2.
        self.stars.get_mut(&tr.v[1]).unwrap().link.delete(tr.v[2]);
        //-- step 3.
        self.stars
            .get_mut(&opposite)
            .unwrap()
            .link
            .insert_after_v(tr.v[0], tr.v[2]);
        //-- step 4.
        self.stars.get_mut(&tr.v[2]).unwrap().link.delete(tr.v[1]);
        //-- make 2 triangles to return (to stack)
        let ret0 = Triangle {
            v: [tr.v[0], tr.v[1], opposite],
        };
        let ret1 = Triangle {
            v: [tr.v[0], opposite, tr.v[2]],
        };
        (ret0, ret1)
    }

    fn get_opposite_vertex(&self, tr: &Triangle) -> usize {
        if self.stars[&tr.v[2]].link.is_empty()
            || self.stars[&tr.v[2]].link.get_next_vertex(tr.v[1]) == None
        {
            return usize::MAX;
        }
        self.stars[&tr.v[2]].link.get_next_vertex(tr.v[1]).unwrap()
    }

    /// Returns a Vec<Vec<f64>> of all the vertices (including the infinite one)
    pub fn all_vertices(&self) -> Vec<Vec<f64>> {
        let mut pts: Vec<Vec<f64>> = Vec::with_capacity(self.stars.len() - 1);

        for key in self.get_star_keys_sorted() {
            pts.push(self.stars.get(&key).unwrap().pt.to_vec());
        }
        pts
    }

    pub fn get_star_keys_sorted(&self) -> Vec<usize> {
        let mut keys: Vec<usize> = self.stars.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn write_stars_obj(&mut self, finalize: bool) {

        for key in self.get_star_keys_sorted() {

            if !self.stars[&key].written
                && (self.can_vertex_be_removed(key)
                || (finalize && self.vertex_exists(key) && !self.stars[&key].link.is_empty()))
            {
                let mut current_star = self.stars.get_mut(&key).unwrap();

                if current_star.local_id == 0 {
                    // Local_id not set yet
                    current_star.local_id = self.global_id;
                    self.global_id += 1;
                }

                io::stdout().write_all(
                    &format!("v {} {} {}\n", current_star.pt[0], current_star.pt[1], current_star.pt[2]).as_bytes(),
                ).expect("");

                current_star.written = true;
            }
        }

        let mut to_remove: Vec<usize> = Vec::new();

        for key in self.get_star_keys_sorted() {
            if self.stars[&key].written {

                for neighbor in self.stars[&key].link.iter() {
                    // Neighbor not completed yet, so don't delete me yet pls
                    if !self.vertex_exists(*neighbor) || !self.stars[&neighbor].written {
                        continue;
                    }
                }

                let mut can_be_removed: bool = false;

                //-- reconstruct triangles
                for (j, value) in self.stars[&key].link.iter().enumerate() {
                    if key < *value {
                        let k = self.stars[&key].link[self.stars[&key].link.next_index(j)];

                        if key < k && self.vertex_exists(*value) && self.vertex_exists(k) {

                            let triangle = Triangle {
                                v: [
                                    self.stars[&key].local_id,
                                    self.stars[value].local_id,
                                    self.stars[&k].local_id,
                                ],
                            };

                            if !triangle.is_infinite() {
                                io::stdout().write_all(
                                    &format!("f {} {} {}\n", triangle.v[0], triangle.v[1], triangle.v[2]).as_bytes(),
                                ).expect("");
                                can_be_removed = true;
                            }
                        }
                    }
                }
                if can_be_removed {
                    to_remove.push(key);
                }
            }
        }

        for vertex in to_remove {
            self.stars.remove(&vertex);
        }
    }

    pub fn all_vertex_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = Vec::with_capacity(self.stars.len() - 1);

        let mut keys: Vec<&usize> = self.stars.keys().collect();
        keys.sort();

        for key in keys {
            ids.push(*key);
        }

        ids
    }

    pub fn all_vertex_ids_written(&self, written: bool) -> Vec<usize> {
        let mut ids: Vec<usize> = Vec::with_capacity(self.stars.len() - 1);

        for key in self.get_star_keys_sorted() {
            // println!("{} can be removed: {}, and is written: {}", key, self.can_vertex_be_removed(key), self.stars[&key].written);
            if self.stars[&key].written == written && self.can_vertex_be_removed(key) {
                ids.push(key);
            }
        }

        ids
    }

    /// Returns a <Vec<usize> of all the finite edges (implicitly grouped by 2)
    pub fn all_edges(&self) -> Vec<usize> {
        let mut edges: Vec<usize> = Vec::new();
        for i in 1..self.stars.len() {
            for value in self.stars[&i].link.iter() {
                if (*value != 0) && (i < *value) {
                    edges.push(i);
                    edges.push(*value);
                }
            }
        }
        edges
    }

    // pub fn all_unwritten_triangles(&mut self) -> Vec<Triangle> {
    //     let mut triangles: Vec<Triangle> = Vec::new();
    //
    //     // Get the id's of all unwritten stars
    //     for key in self.get_star_keys_sorted() {
    //         let mut is_written = false;
    //
    //         {
    //             let star = self.stars.get(&key).unwrap();
    //
    //             if star.to_write == false {
    //                 continue;
    //             }
    //
    //             //-- reconstruct triangles
    //             for (j, value) in star.link.iter().enumerate() {
    //                 if key < *value {
    //                     let k = star.link[star.link.next_index(j)];
    //
    //                     if key < k && self.vertex_exists(*value) && self.vertex_exists(k) {
    //                         let triangle = Triangle {
    //                             v: [
    //                                 star.local_id,
    //                                 self.stars[value].local_id,
    //                                 self.stars[&k].local_id,
    //                             ],
    //                         };
    //
    //                         if !triangle.is_infinite() {
    //                             triangles.push(triangle);
    //                             is_written = true;
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //         if is_written {
    //             {
    //                 // Shorten lifetime of mutable self.stars
    //                 let mut star = self.stars.get_mut(&key).unwrap();
    //                 star.written = is_written;
    //             }
    //         }
    //     }
    //     triangles
    // }

    /// Returns a <Vec<Triangle> of all the finite triangles (including the infinite one)
    pub fn all_triangles(&self) -> Vec<Triangle> {
        let mut trs: Vec<Triangle> = Vec::new();
        for (i, star) in &self.stars {
            //-- reconstruct triangles
            for (j, value) in star.link.iter().enumerate() {
                if i < value {
                    // let k = star.l[self.nexti(star.link.len(), j)];
                    let k = star.link[star.link.next_index(j)];
                    if i < &k {
                        let tr = Triangle { v: [*i, *value, k] };
                        if tr.is_infinite() == false {
                            // println!("{}", tr);
                            trs.push(tr);
                        }
                    }
                }
            }
        }
        trs
    }

    /// Validates the Delaunay triangulation:
    /// (1) checks each triangle against each vertex (circumcircle tests); very slow
    /// (2) checks whether the convex hull is really convex
    pub fn is_valid(&mut self) -> bool {
        self.is_valid_ch_convex() && self.is_valid_circumcircle()
    }

    fn is_valid_circumcircle(&mut self) -> bool {
        let mut re = true;
        let trs = self.all_triangles();
        for tr in trs.iter() {
            for i in 1..self.stars.len() {
                if geom::incircle(
                    &self.stars[&tr.v[0]].pt,
                    &self.stars[&tr.v[1]].pt,
                    &self.stars[&tr.v[2]].pt,
                    &self.stars[&i].pt,
                    self.robust_predicates,
                ) > 0
                {
                    println!("NOT DELAUNAY FFS!");
                    println!("{} with {}", tr, i);
                    re = false
                }
            }
        }
        re
    }

    fn is_valid_ch_convex(&self) -> bool {
        let mut re = true;
        let ch = self.convex_hull();
        for i in 0..ch.len() {
            if geom::orient2d(
                &self.stars[&ch[i % ch.len()]].pt,
                &self.stars[&ch[(i + 1) % ch.len()]].pt,
                &self.stars[&ch[(i + 2) % ch.len()]].pt,
                self.robust_predicates,
            ) == -1
            {
                re = false;
                break;
            }
        }
        // if re == false {
        //     println!("CONVEX NOT CONVEX");
        // }
        return re;
    }

    fn remove_on_convex_hull(&mut self, v: usize) -> Result<usize, &'static str> {
        println!("!!! REMOVE ON CONVEX HULL");
        let mut adjs: Vec<usize> = Vec::new();
        //-- necessary because assumptions below for start-end line on CH
        self.stars.get_mut(&v).unwrap().link.infinite_first();
        for each in self.stars[&v].link.iter() {
            adjs.push(*each);
        }
        // println!("adjs: {:?}", adjs);
        let mut cur: usize = 0;
        //-- 1. find and create finite triangles only
        let mut nadjs = adjs.len();
        let mut steps = 0;
        while adjs.len() > 3 {
            //-- control the loops to avoid infinite loop, when all options in a temp
            //-- star have been tried it's because we're stuck (and done actually)
            if steps == nadjs {
                break;
            }
            if adjs.len() == nadjs {
                steps += 1;
            } else {
                nadjs = adjs.len();
                steps = 0;
            }
            //-- define the ear
            let a = cur % adjs.len();
            let b = (cur + 1) % adjs.len();
            let c = (cur + 2) % adjs.len();
            // println!("cur ear--> {:?} {}/{}/{}", adjs, a, b, c);
            if adjs[a] == 0 || adjs[b] == 0 || adjs[c] == 0 {
                //-- do not process infinite ear
                cur += 1;
                continue;
            }
            if (geom::orient2d(
                &self.stars[&adjs[a]].pt,
                &self.stars[&adjs[b]].pt,
                &self.stars[&adjs[c]].pt,
                self.robust_predicates,
            ) == 1)
                && (geom::orient2d(
                    &self.stars[&adjs[a]].pt,
                    &self.stars[&adjs[c]].pt,
                    &self.stars[&v].pt,
                    self.robust_predicates,
                ) >= 0)
            {
                // println!("ear {}-{}-{}", adjs[a], adjs[b], adjs[c]);
                //-- test incircle with all other vertices in the "hole"
                let cur2 = cur + 3;
                let mut isdel = true;
                for i in 0..adjs.len() - 3 {
                    // println!("test ear with {}", adjs[(cur2 + i) % adjs.len()]);
                    if adjs[(cur2 + i) % adjs.len()] != 0
                        && geom::incircle(
                            &self.stars[&adjs[a]].pt,
                            &self.stars[&adjs[b]].pt,
                            &self.stars[&adjs[c]].pt,
                            &self.stars[&adjs[(cur2 + i) % adjs.len()]].pt,
                            self.robust_predicates,
                        ) > 0
                    {
                        isdel = false;
                        break;
                    }
                }
                if isdel == true {
                    // println!("flip22");
                    let t = Triangle {
                        v: [adjs[a], adjs[b], v],
                    };
                    self.flip22(&t, adjs[c]);
                    adjs.remove((cur + 1) % adjs.len());
                }
            }
            cur += 1;
        }
        //-- flip31 to remove the vertex
        if adjs.len() == 3 {
            self.flip31(v);
            return Ok(self.stars.len() - 1);
        } else {
            //-- convex part is filled, and we need to apply a special "flip"
            //-- to delete the vertex v and its incident edges
            // println!("FLIP-FOR-CH");
            self.stars.get_mut(&adjs[1]).unwrap().link.delete(v);
            self.stars
                .get_mut(&adjs.last().unwrap())
                .unwrap()
                .link
                .delete(v);
            for i in 2..(adjs.len() - 1) {
                self.stars.get_mut(&adjs[i]).unwrap().link.replace(v, 0);
                self.stars.get_mut(&adjs[i]).unwrap().link.infinite_first();
            }
            let mut prev = v;
            for i in 2..(adjs.len() - 1) {
                self.stars
                    .get_mut(&0)
                    .unwrap()
                    .link
                    .insert_after_v(adjs[i], prev);
                prev = adjs[i];
            }
            self.stars.get_mut(&adjs[0]).unwrap().link.delete(v);
            self.stars.get_mut(&v).unwrap().link.clear();
            self.stars.remove(&v);
            // self.stars.get_mut(&v).unwrap().pt[0] = -999.9;
            // self.stars.get_mut(&v).unwrap().pt[1] = -999.9;
            // self.stars.get_mut(&v).unwrap().pt[2] = -999.9;
            // self.removed_indices.push(v);

            for i in 0..1000 {
                if adjs[i] != 0 {
                    self.cur = adjs[0];
                    break;
                }
            }
            // if adjs[0] != 0 {
            //     self.cur = adjs[0];
            // } else {
            //     self.cur = adjs[1];
            // }
            return Ok(self.stars.len() - 1);
        }
    }

    pub fn remove(&mut self, v: usize) -> Result<usize, &'static str> {
        // println!("REMOVE vertex {}", v);
        if v == 0 {
            return Err("Cannot remove the infinite vertex");
        }

        if !self.vertex_exists(v) {
            return Err("Vertex does not exist");
        }

        if self.is_vertex_convex_hull(v) {
            return self.remove_on_convex_hull(v);
        }

        let mut adjs: Vec<usize> = Vec::new();
        for each in self.stars[&v].link.iter() {
            adjs.push(*each);
        }

        // println!("adjs: {:?}", adjs);
        let mut cur: usize = 0;

        let mut count: usize = 0;

        while adjs.len() > 3 {
            let a = cur % adjs.len();
            let b = (cur + 1) % adjs.len();
            let c = (cur + 2) % adjs.len();

            if count > 100 {
                // println!("Hit failure, exiting");
                // println!("Writing to GeoJSON!");
                self.write_geojson_triangles("data\\loop_failure_triangles.json".to_string())
                    .expect("Failure when writing to GeoJSON!");
                // std::process::exit(0x0100);
                // Found a vertex we actually can't remove?
                return Ok(self.stars.len() - 1);
            }

            // println!("cur ear--> {:?} {}/{}/{}", adjs, a, b, c);

            if geom::orient2d(
                &self.stars[&adjs[a]].pt,
                &self.stars[&adjs[b]].pt,
                &self.stars[&adjs[c]].pt,
                self.robust_predicates,
            ) == 1
                && geom::orient2d(
                    &self.stars[&adjs[a]].pt,
                    &self.stars[&adjs[c]].pt,
                    &self.stars[&v].pt,
                    self.robust_predicates,
                ) >= 0
            {
                // println!("ear {}-{}-{}", adjs[a], adjs[b], adjs[c]);
                //-- test incircle with all other vertices in the "hole"
                let cur2 = cur + 3;
                let mut isdel = true;

                for i in 0..adjs.len() - 3 {
                    // println!("test ear with {}", adjs[(cur2 + i) % adjs.len()]);

                    // println!("Incircle: {}", geom::incircle(
                    //     &self.stars[&adjs[a]].pt,
                    //     &self.stars[&adjs[b]].pt,
                    //     &self.stars[&adjs[c]].pt,
                    //     &self.stars[&adjs[(cur2 + i) % adjs.len()]].pt,
                    //     self.robust_predicates
                    // ));

                    if geom::incircle(
                        &self.stars[&adjs[a]].pt,
                        &self.stars[&adjs[b]].pt,
                        &self.stars[&adjs[c]].pt,
                        &self.stars[&adjs[(cur2 + i) % adjs.len()]].pt,
                        self.robust_predicates,
                    ) > 0
                    {
                        isdel = false;
                        break;
                    }
                }

                if isdel {
                    // println!("flip22");
                    let t = Triangle {
                        v: [adjs[a], adjs[b], v],
                    };
                    self.flip22(&t, adjs[c]);
                    adjs.remove((cur + 1) % adjs.len());
                }
            }
            cur = cur + 1;
            count += 1;
        }
        //-- flip31 to remove the vertex
        self.flip31(v);
        return Ok(self.stars.len() - 1);
    }

    /// write an OBJ file to disk
    pub fn write_obj(&mut self, path: String, twod: bool) -> std::io::Result<()> {
        let trs = self.all_triangles();
        let mut f = File::create(path)?;
        let mut s = String::new();
        for i in 1..self.stars.len() {
            if twod == true {
                s.push_str(&format!(
                    "v {} {} {}\n",
                    self.stars[&i].pt[0], self.stars[&i].pt[1], 0
                ));
            } else {
                s.push_str(&format!(
                    "v {} {} {}\n",
                    self.stars[&i].pt[0], self.stars[&i].pt[1], self.stars[&i].pt[2]
                ));
            }
        }
        write!(f, "{}", s).unwrap();
        let mut s = String::new();
        for tr in trs.iter() {
            s.push_str(&format!("f {} {} {}\n", tr.v[0], tr.v[1], tr.v[2]));
        }
        write!(f, "{}", s).unwrap();
        // println!("write fobj: {:.2?}", starttime.elapsed());
        Ok(())
    }

    /// write a GeoJSON file of the triangles/vertices to disk
    pub fn write_geojson_triangles(&self, path: String) -> std::io::Result<()> {
        let mut fc = FeatureCollection {
            bbox: None,
            features: vec![],
            foreign_members: None,
        };
        //-- vertices
        for (i, star) in &self.stars {
            if *i == 0 {
                continue;
            }
            let pt = Geometry::new(Value::Point(vec![star.pt[0], star.pt[1]]));
            let mut attributes = Map::new();
            attributes.insert(String::from("id"), to_value(i.to_string()).unwrap());
            attributes.insert(
                String::from("written"),
                serde_json::value::Value::Bool(star.written),
            );
            let f = Feature {
                bbox: None,
                geometry: Some(pt),
                id: None,
                properties: Some(attributes),
                foreign_members: None,
            };
            fc.features.push(f);
        }
        //-- triangles
        for (i, star) in &self.stars {
            for (j, value) in star.link.iter().enumerate() {
                if (i < value) && (self.stars.contains_key(&value) == true) {
                    let k = star.link[star.link.next_index(j)];
                    if (i < &k) && (self.stars.contains_key(&k) == true) {
                        let tr = Triangle { v: [*i, *value, k] };
                        if tr.is_infinite() == false {
                            let mut l: Vec<Vec<Vec<f64>>> = vec![vec![Vec::with_capacity(1); 4]];
                            l[0][0].push(self.stars[i].pt[0]);
                            l[0][0].push(self.stars[i].pt[1]);
                            l[0][1].push(self.stars[value].pt[0]);
                            l[0][1].push(self.stars[value].pt[1]);
                            l[0][2].push(self.stars[&k].pt[0]);
                            l[0][2].push(self.stars[&k].pt[1]);
                            l[0][3].push(self.stars[i].pt[0]);
                            l[0][3].push(self.stars[i].pt[1]);
                            let gtr = Geometry::new(Value::Polygon(l));
                            // let mut attributes = Map::new();
                            // if self.stars[]
                            // attributes.insert(String::from("active"), to_value();
                            let f = Feature {
                                bbox: None,
                                geometry: Some(gtr),
                                id: None,
                                properties: None, //Some(attributes),
                                foreign_members: None,
                            };
                            fc.features.push(f);
                        }
                    }
                }
            }
        }
        //-- write the file to disk
        let mut fo = File::create(path)?;
        write!(fo, "{}", fc.to_string()).unwrap();
        Ok(())
    }

    pub fn printme(&self, _withxyz: bool) -> String {
        let mut s = String::from("**********\n");
        // s.push_str(&format!("#pts: {}\n", self.number_pts()));

        let mut allkeys: Vec<&usize> = self.stars.keys().collect();
        allkeys.sort();
        for each in allkeys {
            let v = self.stars.get(each).unwrap();
            s.push_str(&format!("{}: [", *each));
            for each2 in v.link.iter() {
                s.push_str(&format!("{} - ", each2));
            }
            s.push_str(&format!("]\n"));
        }

        // for (i, p) in &self.stars {
        //     // for (i, p) in self.stars.iter().enumerate() {
        //     // s.push_str(&format!("{}: {}\n", i, self.stars[i].link));
        //     s.push_str(&format!("{}: [", i));
        //     for each in p.link.iter() {
        //         s.push_str(&format!("{} - ", each));
        //     }
        //     s.push_str(&format!("]\n"));
        //     if withxyz == true {
        //         s.push_str(&format!("\t{:?}\n", self.stars[&i].pt));
        //     }
        // }
        s.push_str("**********\n");
        s
    }

    fn vertex_exists(&self, v: usize) -> bool {
        self.stars.contains_key(&v)
    }

    /// Interpolation: nearest/closest neighbour
    /// None if outside the convex hull, other the value
    pub fn interpolate_nn(&self, px: f64, py: f64) -> Option<f64> {
        let re = self.closest_point(px, py);
        if re.is_some() {
            Some(self.stars[&re.unwrap()].pt[2])
        } else {
            None
        }
    }

    /// Interpolation: linear in TIN
    /// None if outside the convex hull, other the value
    pub fn interpolate_tin_linear(&self, px: f64, py: f64) -> Option<f64> {
        let p: [f64; 3] = [px, py, 0.0];
        let tr = self.walk(&p);
        if tr.is_infinite() {
            return None;
        }

        let a0: f64 = geom::area_triangle(&p, &self.stars[&tr.v[1]].pt, &self.stars[&tr.v[2]].pt);
        let a1: f64 = geom::area_triangle(&p, &self.stars[&tr.v[2]].pt, &self.stars[&tr.v[0]].pt);
        let a2: f64 = geom::area_triangle(&p, &self.stars[&tr.v[0]].pt, &self.stars[&tr.v[1]].pt);
        let mut total = 0.;
        total += self.stars[&tr.v[0]].pt[2] * a0;
        total += self.stars[&tr.v[1]].pt[2] * a1;
        total += self.stars[&tr.v[2]].pt[2] * a2;
        Some(total / (a0 + a1 + a2))
    }

    /// Interpolation with Laplace (http://dilbert.engr.ucdavis.edu/~suku/nem/index.html)
    /// (variation of nni with distances instead of stolen areas; faster in practice)
    /// None if outside the convex hull, other the value
    pub fn interpolate_laplace(&mut self, px: f64, py: f64) -> Option<f64> {
        if self.locate(px, py).is_none() {
            return None;
        }
        let re = self.insert_one_pt(px, py, 0., 0);
        let pi: usize;
        if re.is_ok() {
            pi = re.unwrap();
        } else {
            pi = re.unwrap_err();
        }
        let l = &self.stars[&pi].link;
        let mut centres: Vec<Vec<f64>> = Vec::new();
        for (i, v) in l.iter().enumerate() {
            let j = l.next_index(i);
            centres.push(geom::circle_centre(
                &self.stars[&pi].pt,
                &self.stars[&v].pt,
                &self.stars[&l[j]].pt,
            ));
        }
        let mut weights: Vec<f64> = Vec::new();
        for (i, v) in l.iter().enumerate() {
            // fetch 2 voronoi centres
            let e = geom::distance2d(&centres[i], &centres[l.prev_index(i)]);
            let w = geom::distance2d(&self.stars[&pi].pt, &self.stars[&v].pt);
            weights.push(e / w);
        }
        let mut z: f64 = 0.0;
        for (i, v) in l.iter().enumerate() {
            z += weights[i] * self.stars[&v].pt[2];
        }
        let sumweights: f64 = weights.iter().sum();
        //-- delete the interpolation location point
        let _rr = self.remove(pi);
        Some(z / sumweights)
    }
}

impl fmt::Display for Triangulation {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("======== TRIANGULATION ========\n")?;
        fmt.write_str(&format!("# vertices: {:19}\n", self.number_of_vertices()))?;
        fmt.write_str(&format!("# triangles: {:18}\n", self.number_of_triangles()))?;
        fmt.write_str(&format!(
            "# convex hull: {:16}\n",
            self.number_of_vertices_on_convex_hull()
        ))?;
        fmt.write_str(&format!("---\nrobust: {}\n", self.robust_predicates))?;
        fmt.write_str("===============================\n")?;
        Ok(())
    }
}
