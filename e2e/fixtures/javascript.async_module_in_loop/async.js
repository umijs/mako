let x = await new Promise((resolve)=>{
    resolve("default")
})

let named = await new Promise((resolve)=>{
    resolve("named")
})

export default x;

export {named}
