use host::polynomial;
use methods::POLYNOMIAL_ID;

fn main() {
    let (receipt, _) = polynomial(3);
    receipt.verify(POLYNOMIAL_ID).expect("Verification failed");
}
