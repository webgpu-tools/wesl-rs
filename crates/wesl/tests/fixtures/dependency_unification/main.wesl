import a::{ VERSION as A_VERSION, C_VERSION as A_C_VERSION, D_VERSION as A_D_VERSION };
import b::{ VERSION as B_VERSION, C_VERSION as B_C_VERSION, D_VERSION as B_D_VERSION };

const_assert A_VERSION == 0x010;
const_assert A_C_VERSION == 0x011;
const_assert A_D_VERSION == 0x010;
const_assert B_VERSION == 0x010;
const_assert B_C_VERSION == 0x011;
const_assert B_D_VERSION == 0x020;

@fragment
fn main() {
}
