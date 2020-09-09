# r-mmdc

Rust port of the mmdc tool, originally published here: [https://github.com/FrankBau/mmdc](https://github.com/FrankBau/mmdc)
Why? Because why not!


## Build for target
Buildable for target systems with meta-rust yocto layer ([https://github.com/meta-rust/meta-rust](https://github.com/meta-rust/meta-rust)) just add it to your bblayer.conf.

### Create Recipe
Install the `cargo bitbake` utility ([https://github.com/meta-rust/cargo-bitbake](https://github.com/meta-rust/cargo-bitbake)):

    cargo install cargo-bitbake

and then run

    cargo bitbake

this will create a `r-mmdc_${PV}.bb` you can use in your custom layer.


