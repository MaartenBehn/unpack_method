
extern crate unpack_method;

struct Test {
    foo: Vec<usize>,
    bar: f32,
}

struct Test2<'a> {
    foo: &'a [usize],
}

impl Test {
    
    #[unpack_method::unpack]
    pub fn func_1(&mut self, a: usize) {
        let i = self.bar;
        print!("{i} {:?}", self.foo);
    } 
}

impl<'a> Test2<'a> {
    
    #[unpack_method::unpack]
    pub fn func_2(&mut self) {
        print!("{:?}", self.foo);
    } 
}

#[test]
fn test() {
    let mut test = Test{
        foo: vec![],
        bar: 2.0,
    };

    test.func_1(0);
    Test::func_1_unpacked(&mut test.bar, &mut test.foo, 0);

    let mut test2 = Test2 {
        foo: &test.foo,
    };

    test2.func_2();
    Test2::func_2_unpacked(&test2.foo);
}
