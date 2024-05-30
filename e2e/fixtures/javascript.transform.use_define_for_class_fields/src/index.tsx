class T{
    a;
    b(){
        return this.a
    }
}

it('define the field with descriptor by default',()=>{
    let t = new T();
    expect(Object.getOwnPropertyDescriptor(t,'a')).toStrictEqual({
      value: undefined,
      writable: true,
      enumerable: true,
      configurable: true
    });
});




