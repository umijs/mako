// interface Person {
//     name: string;
//     age: number;
// }
// // 等同于（推荐使用type）
// type Person = {
//     name: string;
//     age: number;
// }

// let tom: Person = {
//     name: 'Tom',
//     age: 25
// };
// // 直等，连类型都相同
// const fakeTom = tom;
// // 解构，类型推断
// const jerry = {...tom, name:"Jerry"};

// // 少属性、多属性都是不允许的
// let tom: Person = {
//     name: 'Tom'
// };
// // error TS2322: Type '{ name: string; }' is not assignable to type 'Person'.
// //   Property 'age' is missing in type '{ name: string; }'.
// let tom: Person = {
//     name: 'Tom',
//     age: 25,
//     gender: 'male'
// };
// // error TS2322: Type '{ name: string; age: number; gender: string; }' is not assignable to type 'Person'.
// //   Object literal may only specify known properties, and 'gender' does not exist in type 'Person'.

// // 少些属性
// type Person = {
//     name?: string; // 可选属性
//     age?: number; // 可选属性
// }
// // 多些属性
// type Person = {
//     name?: string;
//     age?: number;
//   	[key:string]: any; // 一旦定义了任意属性，那么确定属性和可选属性的类型都必须是它的类型的子集
//   	// [key:string]: string|number;
// }
// // 一个接口中只能定义一个任意属性。如果接口中有多个类型的属性，则可以在任意属性中使用联合类型：

// 只读属性
type Person = {
    readonly id: number; // 字段只能在【创建】的时候被赋值
    name: string;
    age?: number;
    [propName: string]: any;
}
let tom: Person = {
    id: 89757, // 创建时赋值 ok
    name: 'Tom',
    gender: 'male'
};
// tom.id = 9527; 
// error TS2540: Cannot assign to 'id' because it is a constant or a read-only property.