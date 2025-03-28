
extern crate unpack_methode;

struct Test {
    foo: Vec<usize>,
    bar: f32,
}

impl Test {
    
    #[unpack_methode::unpack]
    pub fn func_one(&mut self, a: usize) {
        let i = self.bar;
        print!("{i} {:?}", self.foo);
    } 
}

#[test]
fn test() {
    let mut test = Test{
        foo: vec![],
        bar: 2.0,
    };

    test.func_one(0);

    Test::func_one_unpacked(&mut test.bar, &mut test.foo, 0);
}
