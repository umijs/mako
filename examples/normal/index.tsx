import React from "react";
import ReactDOM from "react-dom/client";

import { foo } from "./foo";
import UmiLogo from "./assets/umi-logo.png";

function App() {
	return <div>
		Hello {foo}
		<img src={UmiLogo} />
	</div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
