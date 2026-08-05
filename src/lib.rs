pub mod deep_ink_nodes {
    include!(concat!(env!("OUT_DIR"), "/deep_ink_nodes.rs"));
}

// mod deep_ink_queries {
//     include!(concat!(env!("OUT_DIR"), "/deep_ink_queries.rs"));
// }

pub mod ink_nodes {
    include!(concat!(env!("OUT_DIR"), "/ink_nodes.rs"));
}

// mod ink_queries {
//     include!(concat!(env!("OUT_DIR"), "/ink_queries.rs"));
// }

pub mod document_sync;
