const f = () => {};
console.log(f);


const a =  {
  default : class {

  }
}
it('class with obj.default should work ',()=>{
  const b = new a.default()
  console.log(b)
})

