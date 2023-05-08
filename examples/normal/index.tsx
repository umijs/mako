import * as React from 'react';
import * as ReactDOM from 'react-dom/client';

import { foo } from './foo';
import { bar } from './bar';
import { name, description } from './desc.json';
import UmiLogo from './assets/umi-logo.png';
import MailchimpUnsplash from './assets/mailchimp-unsplash.jpg';
import './index.css';

function App() {
  return (
    <div>
      <div className="title">Hello {foo}</div>
      <div className="desc">Hello {bar}</div>
      <img src={UmiLogo} />
      <div>
        <img style={{ width: 200 }} src={MailchimpUnsplash} alt="unsplash big image" />
      </div>
      <p>
        This is {name}, {description}
      </p>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
