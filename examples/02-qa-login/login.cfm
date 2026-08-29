<!doctype html>
<html><head><meta charset="utf-8"><title>Acme Admin — Sign in</title>
<style>
  body{font-family:system-ui,sans-serif;background:#f3f4f6;margin:0;display:flex;min-height:100vh;align-items:center;justify-content:center}
  form{background:#fff;padding:32px;border-radius:12px;box-shadow:0 4px 24px rgba(0,0,0,.08);width:320px}
  h1{margin:0 0 20px;font-size:20px}
  label{display:block;font-size:13px;margin:12px 0 4px;color:#374151}
  input,button{width:100%;box-sizing:border-box;padding:10px;border:1px solid #d1d5db;border-radius:6px;font-size:14px}
  button{background:#2563eb;color:#fff;border:0;margin-top:16px;cursor:pointer}
  .error{color:#b91c1c;font-size:13px;margin-top:10px;display:none}
  .dashboard{background:#fff;padding:32px;border-radius:12px;width:520px;box-shadow:0 4px 24px rgba(0,0,0,.08)}
  .dashboard h1{color:#065f46}
  .kpi{display:inline-block;margin:8px 16px 0 0;padding:12px 16px;background:#ecfdf5;border-radius:8px}
  .kpi b{display:block;font-size:22px}
</style></head>
<body>
<form id="login" onsubmit="return false">
  <h1>Sign in</h1>
  <label for="username">Username</label><input id="username" autocomplete="off">
  <label for="password">Password</label><input id="password" type="password">
  <button id="loginButton" type="submit">Sign in</button>
  <div class="error" id="error">Invalid username or password.</div>
</form>
<script>
  // A deliberately client-side "app": the dashboard only exists after JS runs.
  document.getElementById('loginButton').addEventListener('click', function () {
    var u = document.getElementById('username').value, p = document.getElementById('password').value;
    if (u === 'sysadmin' && p === 'password') {
      setTimeout(function () {            // simulate a round-trip
        document.body.innerHTML =
          '<section class="dashboard"><h1>Welcome back, ' + u + '</h1>' +
          '<p>Signed in at <time id="when">' + new Date().toISOString() + '</time></p>' +
          '<div class="kpi"><b>42</b>open tickets</div>' +
          '<div class="kpi"><b>7</b>deploys today</div>' +
          '<div class="kpi"><b>99.98%</b>uptime</div></section>';
      }, 300);
    } else {
      document.getElementById('error').style.display = 'block';
    }
  });
</script>
</body></html>
