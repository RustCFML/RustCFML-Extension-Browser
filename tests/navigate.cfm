<cfscript>
// Navigations the page makes itself: a link, a form, a script. Obscura queues
// them and the extension has to commit them; before it did, every one of these
// left the page where it was.
here = "file://#getDirectoryFromPath( getCurrentTemplatePath() )#";
fails = 0;
function check( label, got, want ) {
  if ( got == want ) { writeOutput( "  ok    " & label & chr(10) ); }
  else { writeOutput( "  FAIL  " & label & " -> got [" & got & "], want [" & want & "]" & chr(10) ); fails++; }
}

p = Browser().newPage().goto( here & "nav.html" );
p.click( "##link" );
check( "click(a) navigates", p.title(), "two" );
p.back();
check( "back() returns from a clicked link", p.title(), "nav" );
p.forward();
check( "forward() goes to it again", p.title(), "two" );
p.back();

p.fill( "##q", "hello" ).click( "##send" );
check( "a submitted form navigates", p.title(), "two" );
check( "with its fields in the URL", listLast( p.url(), "?" ), "q=hello" );
p.back();

p.evaluate( "location.href = 'two.html'" );
check( "assigning location navigates", p.title(), "two" );
p.back();

p.click( "##later" ).waitForSelector( "h1", 5000 );
check( "a delayed navigation lands while waiting", p.title(), "two" );
p.back();

p.click( "##frag" );
check( "a fragment link stays on the document", p.title(), "nav" );

check( "history.length sees every entry", p.evaluate( "history.length" ) >= 2, true );
p.close();

writeOutput( fails ? "SOME FAILED" & chr(10) : "ALL NAVIGATION OK" & chr(10) );
</cfscript>
