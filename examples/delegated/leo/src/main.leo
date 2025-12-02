program delegated_proving_test.aleo {
    transition divide(a: u64, b: u64, c: u64, d: u64) -> u64 {
        let result1: u64 = a / b;
        let result2: u64 = result1 / c;
        let result3: u64 = result2 / d;
        let result4: u64 = a / d;
        let result5: u64 = result4 / b;
        let final_result: u64 = result3 + result5;
        return final_result;
    }

    @noupgrade
    async constructor() {}
}
