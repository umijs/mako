import "./index.less";
import "./index.sass";

// @ts-ignore
import styleLess from "./style.module.less";
// @ts-ignore
import styleSass from "./style.module.sass";

document.body.innerHTML = `<div>Style loader example
  <div class=${JSON.stringify(styleLess.less)}>Less Module</div>
  <div class=${JSON.stringify(styleSass.sass)}>Sass Module</div>
</div>`;
