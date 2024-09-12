import assert from "assert";
import 'zx/globals';

const port = '8000';

async function fetchHome() {
    const res = await fetch(`http://localhost:${port}`);
    assert(res.status === 200, "should start umi dev server");
    assert(res.headers.get("content-type").includes("text/html"), "should return html");
    return res;
}

async function fetchApi() {
    const res = await fetch(`http://localhost:${port}/api/users`);
    assert(res.status === 200, "should return 200");
    assert(res.headers.get("content-type").includes("application/json"), "should return json");
    return res;
}

async function fetchHTMLFile() {
    const res = await fetch(`http://localhost:${port}/test.html`);
    assert(res.status === 200, "should return 200");
    assert(res.headers.get("content-type").includes("text/html"), "should return html");
    return res;
}

async function fetchCSSFile() {
    const res = await fetch(`http://localhost:${port}/umi.css`);
    assert(res.status === 200, "should return 200");
    assert(res.headers.get("content-type").includes("text/css"), "should return css");
    return res;
}

export default async function () {
    await fetchHome();
    await fetchApi();
    await fetchHTMLFile();
    await fetchCSSFile();
}
