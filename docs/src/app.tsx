import css from './app.module.css';
import { Outlet, NavLink } from 'react-router';

export const Home = () => (
  <div className={css.home}>
    <h2>dbschema</h2>
    <p>
      A Rust CLI to define database schemas in a small HCL dialect and generate
      idempotent SQL migrations.
    </p>
  </div>
);

const App = () => (
  <div className={css.body}>
    <header>
      <h1><NavLink to="/">dbschema</NavLink></h1>
      <nav>
        <NavLink to="/docs">Docs</NavLink>
      </nav>
    </header>
    <Outlet />
    <footer>
      <hr />
      <span>
        Check out the project on <a href="https://github.com/theknarf-experiments/dbschema">GitHub</a>
      </span>
    </footer>
  </div>
);

export default App;
