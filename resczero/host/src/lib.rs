use methods::POLYNOMIAL_ELF;
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};

// Compute the polynomial y = x^3 + x + t in zkVM
pub fn polynomial(x: u64) -> (Receipt, u64) {
    let env = ExecutorEnv::builder()
        // send x to the guest
        .write(&x)
        .unwrap()
        .build()
        .unwrap();

    let prover = default_prover();
    let receipt = prover.prove(env, POLYNOMIAL_ELF).unwrap().receipt;

    let output: u64 = receipt.journal.decode().expect(
        "Failed to decode the output from the journal. This is likely a bug in the guest program.",
    );

    println!("I know that result is {}, and I can prove it!", output);

    (receipt, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polynomial() {
        const TEST_X: u64 = 3;
        let (_, output) = polynomial(3);
        assert_eq!(
            output,
            TEST_X * TEST_X * TEST_X + TEST_X + 5,
            "The output is not correct. The polynomial is y = x^3 + x + 5"
        );
    }
}
