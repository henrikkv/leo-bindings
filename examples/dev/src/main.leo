program dev.aleo {
    @noupgrade
    async constructor() {}
    
    transition main(public a: u32, public b: u32) -> public u32 {
        let c: u32 = a + b;
        return c;
    }
}
