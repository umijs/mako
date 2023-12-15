let myFavoriteNumber: string | number;

// 赋值时依旧会进行类型推论
myFavoriteNumber = "seven";
console.log(myFavoriteNumber.length); // 5
myFavoriteNumber = 7;
// console.log(myFavoriteNumber.length); // 编译时报错
// error TS2339: Property 'length' does not exist on type 'number'.

// 只能访问此联合类型的所有类型里共有的属性或方法
function getString(something: string | number): string {
  return something.toString(); // 共有方法 ok
}
// function getLength(something: string | number): number {
//   return something.length; // number没有改属性
// }
// error TS2339: Property 'length' does not exist on type 'string | number'.
//   Property 'length' does not exist on type 'number'.

// 常用类型定义
type JumaoType = "fat" | "very_fat" | "very_very_fat";
const myJumao: JumaoType = "fat";
type IMyJumao = {
  type: JumaoType;
};
