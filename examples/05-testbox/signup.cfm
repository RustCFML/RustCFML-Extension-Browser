<!doctype html>
<html><head><meta charset="utf-8"><title>Acme — Create your account</title>
<style>
  body{font-family:system-ui,sans-serif;background:#f4f5f7;margin:0;padding:40px;color:#111}
  .wrap{max-width:460px;margin:0 auto;background:#fff;padding:28px;border-radius:12px;box-shadow:0 4px 24px rgba(0,0,0,.07)}
  h1{margin:0 0 18px;font-size:21px}
  label{display:block;font-size:13px;margin:14px 0 4px;color:#374151}
  input,select,button{width:100%;box-sizing:border-box;padding:10px;border:1px solid #d1d5db;border-radius:6px;font-size:14px}
  button{background:#1d4ed8;color:#fff;border:0;margin-top:20px;cursor:pointer;font-weight:600}
  .err{color:#b91c1c;font-size:12px;margin-top:4px;display:none}
  .summary{margin-top:22px;padding:18px;background:#ecfdf5;border:1px solid #a7f3d0;border-radius:8px}
  .summary h2{margin:0 0 10px;font-size:16px;color:#065f46}
  .summary dt{font-size:12px;color:#047857;margin-top:8px}
  .summary dd{margin:0;font-weight:600}
</style></head>
<body>
<div class="wrap">
  <h1>Create your account</h1>
  <form id="signup" novalidate>
    <label for="fullName">Full name</label>
    <input id="fullName" name="fullName" autocomplete="off">
    <div class="err" data-error-for="fullName">Please tell us your name.</div>

    <label for="email">Email</label>
    <input id="email" name="email" autocomplete="off">
    <div class="err" data-error-for="email">That does not look like an email address.</div>

    <label for="plan">Plan</label>
    <select id="plan" name="plan">
      <option value="starter">Starter</option>
      <option value="team">Team</option>
      <option value="enterprise">Enterprise</option>
    </select>

    <button id="submitButton" type="submit">Create account</button>
  </form>
  <div id="result"></div>
</div>

<script>
  // Deliberately client side: none of the output below exists in the HTML that
  // cfhttp would fetch. It is built by JavaScript after the click, which is
  // the whole reason a test needs a real browser.
  var PRICES = { starter: 0, team: 29, enterprise: 99 };

  document.getElementById('signup').addEventListener('submit', function (e) {
    e.preventDefault();

    var name  = document.getElementById('fullName').value.trim();
    var email = document.getElementById('email').value.trim();
    var plan  = document.getElementById('plan').value;

    var problems = { fullName: !name, email: !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email) };
    Object.keys(problems).forEach(function (field) {
      document.querySelector('[data-error-for="' + field + '"]').style.display =
        problems[field] ? 'block' : 'none';
    });
    if (problems.fullName || problems.email) {
      document.getElementById('result').innerHTML = '';
      return;
    }

    setTimeout(function () {          // simulate the round-trip
      document.getElementById('result').innerHTML =
        '<div class="summary" data-testid="summary">' +
          '<h2>Welcome, ' + name + '</h2>' +
          '<dl>' +
            '<dt>Email</dt><dd data-testid="email">' + email + '</dd>' +
            '<dt>Plan</dt><dd data-testid="plan">' + plan.charAt(0).toUpperCase() + plan.slice(1) + '</dd>' +
            '<dt>Monthly</dt><dd data-testid="price">$' + PRICES[plan] + '</dd>' +
          '</dl>' +
        '</div>';
    }, 120);
  });
</script>
</body></html>
