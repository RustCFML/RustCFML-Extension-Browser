<cfscript>
/*  Release smoke test: drive a real page and capture it, entirely offline.
    The page is a local file and the engine bundles its own fonts, so this
    passes or fails on the artifact alone — never on the runner's network or
    font situation. */

if ( !isDefined( "Browser" ) ) {
    throw( message = "Browser() was not registered — the extension did not load" );
}

here = getDirectoryFromPath( getCurrentTemplatePath() );
fileWrite( "#here#smoke.html", '<!doctype html><html><head><style>
    body{margin:0;font-family:sans-serif;background:##1d4ed8;color:##fff}
    h1{padding:40px}
  </style></head><body><h1 id="t">smoke</h1>
  <script>document.getElementById("t").textContent = "js-ran";</script>
  </body></html>' );

page = Browser().newPage().goto( "file://#here#smoke.html" );

// JavaScript executed and the DOM answers.
if ( page.text( "h1" ) neq "js-ran" ) {
    throw( message = "script did not run: h1 is '#page.text( "h1" )#'" );
}
if ( page.evaluate( "1 + 1" ) neq 2 ) {
    throw( message = "evaluate() broken" );
}

// The paint engine painted something. A blank capture is a valid PNG, which
// is why detail=true exists; assert on ink, not on bytes.
shot = page.screenshot( { width = 640, height = 400, detail = true } );
if ( shot.looksUnrendered ) {
    throw( message = "screenshot painted nothing (ink #shot.inkCoverage#)" );
}

// The PDF path produces a PDF.
pdf = page.pdf( { paper = "A4" } );
if ( len( pdf ) lt 1000 ) {
    throw( message = "pdf() produced #len( pdf )# bytes" );
}

f = page.fonts();
page.close();
writeOutput( "loaded ok — text/evaluate/screenshot/pdf all answered" & chr( 10 ) );
writeOutput( "ink coverage: " & numberFormat( shot.inkCoverage * 100, "0.0" ) & "%, pdf: " & len( pdf ) & " bytes" & chr( 10 ) );
</cfscript>
