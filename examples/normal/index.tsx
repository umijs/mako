import * as React from 'react';
import * as ReactDOM from 'react-dom/client';

import { foo } from './foo';
import { bar } from './bar';
import UmiLogo from './assets/umi-logo.png';
import MailchimpUnsplash from './assets/mailchimp-unsplash.jpg';
import './index.css';

const Lazy = React.lazy(() => import('./lazy'));
function App() {
  return (
    <div>
      <Lazy />
      <div className="title">Hello {foo}</div>
      <div className="title">Hello {bar}</div>
      <img src={UmiLogo} />
      <div>
        <img style={{ width: 200 }} src={MailchimpUnsplash} alt="unsplash big image" />
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
