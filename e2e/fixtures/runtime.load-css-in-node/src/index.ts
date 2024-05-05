import('./foo').then(res => {
    console.log(res.default);
}).catch(e => {
    console.error(e);
});
// import './foo.css';
console.log(1);
