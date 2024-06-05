use halo2_proofs::circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner, Value};
use halo2_proofs::dev::MockProver;
use halo2_proofs::pasta::Fp;
use halo2_proofs::plonk::{
    Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector,
};
use halo2_proofs::poly::Rotation;

trait NumbericInstructions {
    type Num;

    fn load_private(&self, layouter: impl Layouter<Fp>, x: Value<Fp>) -> Result<Self::Num, Error>;
    fn load_constant(&self, layouter: impl Layouter<Fp>, x: Fp) -> Result<Self::Num, Error>;

    fn mul(
        &self,
        layouter: impl Layouter<Fp>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error>;

    fn add(
        &self,
        layouter: impl Layouter<Fp>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error>;

    fn expose_public(
        &self,
        layouter: impl Layouter<Fp>,
        num: Self::Num,
        row: usize,
    ) -> Result<(), Error>;
}

#[derive(Clone, Debug)]
struct FieldConfig {
    advice: [Column<Advice>; 2],
    instance: Column<Instance>,
    s_mul: Selector,
    s_add: Selector,
}

struct FieldChip {
    config: FieldConfig,
}

impl Chip<Fp> for FieldChip {
    type Config = FieldConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl FieldChip {
    fn new(config: FieldConfig) -> Self {
        Self { config }
    }

    fn configure(
        meta: &mut ConstraintSystem<Fp>,
        advice: [Column<Advice>; 2],
        instance: Column<Instance>,
        constant: Column<Fixed>,
    ) -> FieldConfig {
        meta.enable_constant(constant);
        meta.enable_equality(instance);

        for column in &advice {
            meta.enable_equality(*column);
        }

        let s_mul = meta.selector();
        let s_add = meta.selector();
        meta.create_gate("mul|add", |mete| {
            //   y = x^3 + x + 5
            //  +------+------+-------+-------+
            //  | a_0  | a_1  | s_mul | s_add |
            //  +------+------+-------+-------+
            //  |  x   |  x   |   1   |   0   |
            //  | x^2  |  x   |   1   |   0   |
            //  | x^3  |  x   |   0   |   1   |
            //  | x^3  |  5   |   0   |   1   |
            //  +------+------+-------+-------+

            let a_0 = mete.query_advice(advice[0], Rotation::cur());
            let a_1 = mete.query_advice(advice[1], Rotation::cur());
            let out = mete.query_advice(advice[0], Rotation::next());

            let s_mul = mete.query_selector(s_mul);
            let s_add = mete.query_selector(s_add);

            // if s_mul = 0, any value is allowed in a_0, a_1, and out.
            // if s_mul != 0, this constrains a_0 * a_1 = out.
            let s_1 = s_mul * (a_0.clone() * a_1.clone() - out.clone());

            // if s_add = 0, any value is allowed in a_0, a_1, and out.
            // if s_add != 0, this constrains a_0 + a_1 = out.
            let s_2 = s_add * (a_0 + a_1 - out);

            vec![s_1, s_2]
        });

        FieldConfig {
            advice,
            instance,
            s_mul,
            s_add,
        }
    }
}

#[derive(Clone, Debug)]
struct Number(AssignedCell<Fp, Fp>);

impl NumbericInstructions for FieldChip {
    type Num = Number;

    fn load_private(
        &self,
        mut layouter: impl Layouter<Fp>,
        x: Value<Fp>,
    ) -> Result<Self::Num, Error> {
        let config = self.config();

        layouter.assign_region(
            || "load private",
            |mut region| {
                region
                    .assign_advice(|| "private input", config.advice[0], 0, || x)
                    .map(Number)
            },
        )
    }

    fn load_constant(&self, mut layouter: impl Layouter<Fp>, x: Fp) -> Result<Self::Num, Error> {
        let config = self.config();

        layouter.assign_region(
            || "load constant",
            |mut region| {
                region
                    .assign_advice_from_constant(|| "constant value", config.advice[0], 0, x)
                    .map(Number)
            },
        )
    }

    fn add(
        &self,
        mut layouter: impl Layouter<Fp>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error> {
        let config = self.config();
        layouter.assign_region(
            || "add",
            |mut region| {
                config.s_add.enable(&mut region, 0)?;
                a.0.copy_advice(|| "s_0", &mut region, config.advice[0], 0)?;
                b.0.copy_advice(|| "s_1", &mut region, config.advice[1], 0)?;

                let value = a.0.value().copied() + b.0.value().copied();

                region
                    .assign_advice(|| "s_0 + s_1", config.advice[0], 1, || value)
                    .map(Number)
            },
        )
    }

    fn mul(
        &self,
        mut layouter: impl Layouter<Fp>,
        a: Self::Num,
        b: Self::Num,
    ) -> Result<Self::Num, Error> {
        let config = self.config();
        layouter.assign_region(
            || "mul",
            |mut region| {
                config.s_mul.enable(&mut region, 0)?;

                a.0.copy_advice(|| "s_0", &mut region, config.advice[0], 0)?;
                b.0.copy_advice(|| "s_1", &mut region, config.advice[1], 0)?;

                let value = a.0.value().copied() * b.0.value();
                region
                    .assign_advice(|| "s_0 * s_1", config.advice[0], 1, || value)
                    .map(Number)
            },
        )
    }

    fn expose_public(
        &self,
        mut layouter: impl Layouter<Fp>,
        num: Self::Num,
        row: usize,
    ) -> Result<(), Error> {
        let config = self.config();
        layouter.constrain_instance(num.0.cell(), config.instance, row)
    }
}

#[derive(Default)]
struct MyCircuit {
    constant: Fp,
    x: Value<Fp>,
}

impl Circuit<Fp> for MyCircuit {
    type Config = FieldConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> FieldConfig {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instance = meta.instance_column();
        let constant = meta.fixed_column();

        FieldChip::configure(meta, advice, instance, constant)
    }

    fn synthesize(
        &self,
        config: FieldConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        // let x = self.x;
        let chip = FieldChip::new(config);

        let x = chip.load_private(layouter.namespace(|| "load x"), self.x)?;
        let constant = chip.load_constant(layouter.namespace(|| "load constant"), self.constant)?;

        let x2 = chip.mul(layouter.namespace(|| "x^2"), x.clone(), x.clone())?;
        let x3 = chip.mul(layouter.namespace(|| "x^3"), x2, x.clone())?;
        let x3_plus_x = chip.add(layouter.namespace(|| "x^3 + x"), x3, x)?;
        let x3_plus_x_plus_5 =
            chip.add(layouter.namespace(|| "x^3 + x + 5"), x3_plus_x, constant)?;

        chip.expose_public(layouter.namespace(|| "expose res"), x3_plus_x_plus_5, 0)
    }
}

fn main() {
    let k = 4;

    let x = Fp::from(3);
    let constant = Fp::from(5);
    let res = Fp::from(35);
    let circuit = MyCircuit {
        constant,
        x: Value::known(x),
    };

    let mut public_inputs = vec![res];
    let prover = MockProver::run(k, &circuit, vec![public_inputs.clone()]).unwrap();
    assert_eq!(prover.verify(), Ok(()));

    public_inputs[0] += Fp::one();
    let prover = MockProver::run(k, &circuit, vec![public_inputs]).unwrap();
    assert!(prover.verify().is_err());

    let x = Fp::from(5);
    let constant = Fp::from(5);
    let res = Fp::from(135);
    let circuit = MyCircuit {
        constant,
        x: Value::known(x),
    };

    let public_inputs = vec![res];
    let prover = MockProver::run(k, &circuit, vec![public_inputs.clone()]).unwrap();
    assert_eq!(prover.verify(), Ok(()));
}
