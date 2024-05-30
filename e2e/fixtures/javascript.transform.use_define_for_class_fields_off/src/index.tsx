class T{
    a;
    b(){
        return this.a
    }
}

it('has no descriptor when disable use define for class fields',()=>{
    let t = new T();
    expect(Object.getOwnPropertyDescriptor(t,'a')).toStrictEqual(undefined);
});




