library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity counter is
    port (
        clk   : in  std_logic;
        rst_n : in  std_logic;
        count : out unsigned(7 downto 0)
    );
end entity counter;

architecture rtl of counter is
    signal cnt_reg : unsigned(7 downto 0) := (others => '0');
begin
    process(clk, rst_n)
    begin
        if rst_n = '0' then
            cnt_reg <= (others => '0');
        elsif rising_edge(clk) then
            cnt_reg <= cnt_reg + 1;
        end if;
    end process;

    count <= cnt_reg;
end architecture rtl;
