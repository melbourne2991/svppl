

fn main() {
    tonic_build::compile_protos("proto/svppl/svppl.proto").unwrap();
}