import React from "react";
import ReactDOM from "react-dom/client";

import { foo } from "./foo";
import UmiLogo from "./assets/umi-logo.png";
import MailchimpUnsplash from "./assets/mailchimp-unsplash.jpg";
import "./index.css";

function App() {
	return <div>
    <div className="title">
		  Hello {foo}
    </div>
		<img src={UmiLogo} />
		<div>
			<img style={{width: 200}} src={MailchimpUnsplash} alt="unsplash big image" />
		</div>
	</div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
