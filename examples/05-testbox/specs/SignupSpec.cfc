/**
 * Browser tests for the signup form, in TestBox.
 *
 * Everything asserted below is built by JavaScript after a click — none of it
 * exists in the HTML that `cfhttp` would fetch. That is the whole reason this
 * needs a real browser rather than a string search over markup.
 *
 * One browser is opened for the suite and reused, because starting one is the
 * expensive part. Each spec navigates fresh, so the specs stay independent.
 */
component extends="testbox.system.BaseSpec" {

    function beforeAll() {
        variables.browser = Browser( { timeout = 30000 } );
        variables.page    = variables.browser.newPage().setViewport( 900, 900 );
        variables.appUrl  = "http://127.0.0.1:" & cgi.server_port & "/examples/05-testbox/signup.cfm";
    }

    function afterAll() {
        variables.page.close();
    }

    function run() {

        describe( "the signup form", function () {

            beforeEach( function () {
                // A fresh load per spec: no state carries over from the last one.
                page.goto( appUrl ).waitForSelector( "##signup", 5000 );
            } );

            it( "shows nothing until it is submitted", function () {
                expect( page.exists( "[data-testid=summary]" ) ).toBeFalse();
            } );

            it( "refuses an empty form and says why", function () {
                page.click( "##submitButton" ).settle( 200 );

                expect( errorShown( "fullName" ) ).toBeTrue( "the name error should be visible" );
                expect( errorShown( "email" ) ).toBeTrue( "the email error should be visible" );
                expect( page.exists( "[data-testid=summary]" ) ).toBeFalse( "nothing should have been created" );
            } );

            it( "rejects an address that is not an email", function () {
                page.fill( "##fullName", "Ada Lovelace" )
                    .fill( "##email", "ada-at-example" )
                    .click( "##submitButton" )
                    .settle( 200 );

                expect( errorShown( "email" ) ).toBeTrue();
                expect( errorShown( "fullName" ) ).toBeFalse( "the name was valid" );
                expect( page.exists( "[data-testid=summary]" ) ).toBeFalse();
            } );

            it( "creates the account and echoes back what was entered", function () {
                page.fill( "##fullName", "Ada Lovelace" )
                    .fill( "##email", "ada@example.com" )
                    .selectOption( "##plan", "team" )
                    .click( "##submitButton" )
                    // The summary is rendered asynchronously; wait for it rather
                    // than sleeping and hoping.
                    .waitForSelector( "[data-testid=summary]", 5000 );

                expect( page.text( "[data-testid=summary] h2" ) ).toBe( "Welcome, Ada Lovelace" );
                expect( page.text( "[data-testid=email]" ) ).toBe( "ada@example.com" );
                expect( page.text( "[data-testid=plan]" ) ).toBe( "Team" );
            } );

            it( "prices each plan correctly", function () {
                var expected = { starter = "$0", team = "$29", enterprise = "$99" };

                for ( var plan in expected ) {
                    page.goto( appUrl ).waitForSelector( "##signup", 5000 )
                        .fill( "##fullName", "Grace Hopper" )
                        .fill( "##email", "grace@example.com" )
                        .selectOption( "##plan", plan )
                        .click( "##submitButton" )
                        .waitForSelector( "[data-testid=summary]", 5000 );

                    expect( page.text( "[data-testid=price]" ) ).toBe( expected[ plan ], "#plan# should cost #expected[ plan ]#" );
                }
            } );

            it( "actually painted something", function () {
                page.fill( "##fullName", "Ada Lovelace" )
                    .fill( "##email", "ada@example.com" )
                    .click( "##submitButton" )
                    .waitForSelector( "[data-testid=summary]", 5000 );

                // A page whose script died before painting still produces a
                // perfectly valid, perfectly blank PNG — so assert on the ink,
                // and keep the image as evidence when it fails.
                var shot = page.screenshot( { width = 900, height = 700, detail = true } );
                if ( shot.looksUnrendered ) {
                    fileWrite( expandPath( "./failed.png" ), shot.image );
                }
                expect( shot.looksUnrendered ).toBeFalse( "the page rendered blank — see failed.png" );
            } );

        } );
    }

    /** Is the inline error for this field on screen? */
    private boolean function errorShown( required string field ) {
        return page.evaluate(
            "getComputedStyle(document.querySelector('[data-error-for=""#arguments.field#""]')).display !== 'none'"
        );
    }

}
