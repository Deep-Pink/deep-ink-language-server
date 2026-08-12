use std::path::{Path, PathBuf};
use std::{env, fs};
use type_sitter_gen::{generate_nodes, generate_queries, super_nodes};

fn main() {
    // Common setup
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo::rerun-if-changed=build.rs");

    // Obligatory: in this and future lines, replace `vendor/path/to/tree-sitter-foobar-lang`
    // with the path to your grammar's folder, relative to the folder containing `Cargo.toml`
    println!("cargo::rerun-if-changed=vendored/tree-sitter-ink");
    println!("cargo::rerun-if-changed=vendored/tree-sitter-deep-pink-ink");
    println!("cargo::rerun-if-changed=ink_queries");
    println!("cargo::rerun-if-changed=deep_ink_queries");

    // To generate nodes
    {
        let deep_pink_path = Path::new("vendored/tree-sitter-deep-pink-ink/src/node-types.json");
        fs::write(
            out_dir.join("deep_ink_nodes.rs"),
            generate_nodes(deep_pink_path).unwrap().into_string(),
        )
        .unwrap();
    }

    {
        let ink_path = Path::new("vendored/tree-sitter-ink/src/node-types.json");
        fs::write(
            out_dir.join("ink_nodes.rs"),
            generate_nodes(ink_path).unwrap().into_string(),
        )
        .unwrap();
    }

    // To generate queries
    // fs::write(
    //     out_dir.join("deep_ink_queries.rs"),
    //     generate_queries(
    //         "deep-ink-queries",
    //         "vendored/tree-sitter-deep-pink-ink",
    //         // Replace with a different `syn::Path` if the nodes don't exist in a subling to `dest_path` named `nodes`
    //         &super_nodes(),
    //         // Replace with `true` if you are using the `yak-sitter` feature (by default, no)
    //         false,
    //     )
    //     .unwrap()
    //     .into_string(),
    // )
    // .unwrap();

    fs::write(
        out_dir.join("ink_queries.rs"),
        generate_queries(
            "ink_queries",
            "vendored/tree-sitter-ink",
            // Replace with a different `syn::Path` if the nodes don't exist in a subling to `dest_path` named `nodes`
            &super_nodes(),
            // Replace with `true` if you are using the `yak-sitter` feature (by default, no)
            false,
        )
        .unwrap()
        .into_string(),
    )
    .unwrap();
}
