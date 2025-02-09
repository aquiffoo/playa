use cmake::Config;

fn main(){
    let dst = Config::new("portaudio")
        .define("PA_USE_ASIO", "ON")
        .build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=portaudio");
    println!("cargo:include={}/include", dst.display());
}
