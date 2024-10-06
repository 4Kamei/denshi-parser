`timescale 1ns / 1ps
`default_nettype none

module eth_scrambler #(
        parameter int NEW_PARAM = 32,
        parameter int DATA_WIDTH = 32
        ) (
        input wire                      i_clk
        );
    
    parameter int Test;

    logic [2:0] te;

    logic       someth;

    logic test;


endmodule

`resetall
